#!/usr/bin/env python3
"""Export the Windows icon set from the 1024x1024 master.

Requires Pillow. Run from the repository root:

    python3 scripts/export-windows-icon.py
"""

from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
MASTER = ROOT / "assets/icons/soundy.png"
SIZES = [(256, 256), (128, 128), (64, 64), (48, 48), (40, 40), (32, 32), (24, 24), (20, 20), (16, 16)]

image = Image.open(MASTER).convert("RGBA")
image.save(ROOT / "assets/icons/Soundy.ico", sizes=SIZES)
print("wrote assets/icons/Soundy.ico")
