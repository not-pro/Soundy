# Soundy icons

The icon is an ear on a keyboard key, in dark orange and amber glass, with a minimal 3D shape and a transparent background.

| File | Use |
| --- | --- |
| `soundy.png` | 1024×1024 master and README illustration |
| `Soundy.icns` | macOS app bundle, Finder and Dock; 16–1024 px |
| `tray.svg` | Simplified ear/key vector source for small sizes |
| `tray-template.png` | macOS menu bar; 36 px displayed at 18 pt, system-tinted template |
| `Soundy.ico` | Windows notification area (tray); 16–256 px multi-size icon |

The macOS bundle includes `Soundy.icns`; the menu bar PNG is embedded in the executable. The Windows ZIP ships `Soundy.ico` next to `Soundy.exe`, which loads it for the notification area and falls back to the stock Windows icon if it is missing.

To regenerate the macOS exports from `soundy.png` and `tray.svg` on macOS:

```sh
swift scripts/export-icons.swift
```

To regenerate the Windows icon from the same master (needs Pillow):

```sh
python3 scripts/export-windows-icon.py
```

The master was generated using the built-in ImageGen tool. Prompt:

> One minimalist 3D application icon: a keyboard keycap with a recognizable stylized ear centered on its top face. Dark burnt-orange and amber glass, a thick rounded-square keycap, restrained refraction and polished bevel highlights. A bold smooth ear glyph, almost frontal view slightly from above, centered square composition, transparent background. No text, surrounding keyboard, extra symbols or watermark.
