# Code signing identity

`soundy-codesign.p12` is a **self-signed** code-signing certificate (password `soundy`)
used by `scripts/bundle.sh` to sign every build with one stable identity.

It is not an Apple Developer ID, it cannot notarize anything, and it grants no Apple
trust. Its only job is to keep the app's **designated requirement** identical across
releases:

```
designated => identifier "dev.soundy.app" and certificate leaf = H"…"
```

That hash comes from the certificate, not from the code. Ad-hoc signing (`codesign -s -`)
derives the requirement from the code instead, so every build looks like a different app.
macOS then drops the **Input Monitoring** approval and the **Open at Login** item after
each Sparkle update, which is the bug this certificate fixes.

Do not regenerate or replace this certificate: a new key means a new identity, and every
existing user loses their permissions one more time. It expires in 2036; keep the same
key when renewing. `cert.pem` is the matching public certificate.
