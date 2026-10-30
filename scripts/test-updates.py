"""Native update probes against signed local feeds; run after bundle.sh on macOS."""

import functools
import http.server
import os
from pathlib import Path
import plistlib
import subprocess
import sys
import tempfile
import threading
import time
import uuid


def run(*args, **kwargs):
    return subprocess.run(args, check=True, **kwargs)


def main():
    key = os.environ["SPARKLE_PRIVATE_KEY"]
    sdk = Path(subprocess.check_output(["sh", "scripts/setup-sparkle.sh"], text=True).strip())
    identifier = "dev.soundy.update-test." + uuid.uuid4().hex
    with tempfile.TemporaryDirectory(prefix="soundy-update-test-") as temporary:
        root = Path(temporary)
        host = root / "host/Soundy.app"
        run("ditto", "dist/Soundy.app", str(host))
        info_path = host / "Contents/Info.plist"
        info = plistlib.loads(info_path.read_bytes())
        info["CFBundleIdentifier"] = identifier
        info["NSAppTransportSecurity"] = {"NSAllowsLocalNetworking": True}
        info_path.write_bytes(plistlib.dumps(info))
        run("codesign", "--force", "--sign", "-", "--identifier", identifier, str(host))

        server_root = root / "server"
        server_root.mkdir()
        requests = []

        class Handler(http.server.SimpleHTTPRequestHandler):
            def log_message(self, *_):
                pass

            def do_GET(self):
                requests.append(self.path)
                super().do_GET()

        server = http.server.ThreadingHTTPServer(
            ("127.0.0.1", 0), functools.partial(Handler, directory=str(server_root))
        )
        thread = threading.Thread(target=server.serve_forever, daemon=True)
        thread.start()
        address = "http://localhost:" + str(server.server_port)
        info["SUFeedURL"] = address + "/future/appcast.xml"
        info_path.write_bytes(plistlib.dumps(info))
        run("codesign", "--force", "--sign", "-", "--identifier", identifier, str(host))
        try:
            for name, version in [("future", "99.0.0"), ("current", info["CFBundleVersion"])]:
                archive_dir = server_root / name
                archive_dir.mkdir()
                app = root / name / "Soundy.app"
                run("ditto", str(host), str(app))
                app_info = dict(info, CFBundleVersion=version, CFBundleShortVersionString=version)
                (app / "Contents/Info.plist").write_bytes(plistlib.dumps(app_info))
                run("codesign", "--force", "--sign", "-", "--identifier", identifier, str(app))
                run("ditto", "-c", "-k", "--sequesterRsrc", "--keepParent", str(app), str(archive_dir / "Soundy.zip"))
                run(
                    str(sdk / "bin/generate_appcast"), "--ed-key-file", "-",
                    "--maximum-deltas", "0", "--download-url-prefix", address + "/" + name + "/",
                    "-o", str(archive_dir / "appcast.xml"), str(archive_dir), input=key, text=True,
                )

            def probe(feed, expected):
                environment = dict(os.environ, SOUNDY_UPDATE_TEST_FEED=feed,
                                   SOUNDY_UPDATE_TEST_EXPECTED=expected)
                run(str(host / "Contents/MacOS/soundy"), "--smoke-test", env=environment, timeout=20)

            # Same application/defaults: both launches must fetch the feed.
            probe(address + "/future/appcast.xml", "99.0.0")
            probe(address + "/future/appcast.xml", "99.0.0")
            assert requests.count("/future/appcast.xml") == 2, requests
            probe(address + "/current/appcast.xml", "none")
            feed = (server_root / "future/appcast.xml").read_bytes()
            tampered = feed.replace(b"99.0.0", b"98.0.0")
            assert tampered != feed
            (server_root / "tampered.xml").write_bytes(tampered)
            probe(address + "/tampered.xml", "none")
            assert not any(path.endswith(".zip") for path in requests), requests
            if "--interactive" in sys.argv:
                print("Interactive install target: " + str(host), flush=True)
                print("Launch this test app, click Update Available and Install Update; waiting up to 3 minutes.", flush=True)
                deadline = time.monotonic() + 180
                while plistlib.loads(info_path.read_bytes())["CFBundleVersion"] != "99.0.0":
                    if time.monotonic() > deadline:
                        raise TimeoutError("isolated app was not updated to 99.0.0")
                    time.sleep(0.5)
                print("OK: signed ZIP installed; isolated app bundle is now 99.0.0", flush=True)
                input("Quit the updated test app, then press Enter to finish the offline check: ")
            server.shutdown()
            server.server_close()
            probe(address + "/offline.xml", "none")
            print("OK: every-launch signed probes, current-version, tampered-feed and offline silence")
        finally:
            server.shutdown()
            server.server_close()
            subprocess.run(["defaults", "delete", identifier], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)


if __name__ == "__main__":
    main()
