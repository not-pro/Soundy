# Soundy icons

The icon is an ear on a keyboard key, in dark orange and amber glass, with a minimal 3D shape and a transparent background.

| File | Use |
| --- | --- |
| `soundy.png` | 1024×1024 master and README illustration |
| `Soundy.icns` | macOS app bundle, Finder and Dock; 16–1024 px |
| `Soundy.ico` | Windows application/taskbar; 16–256 px |
| `tray.svg` | Simplified ear/key vector source for small sizes |
| `tray-template.png` | macOS menu bar; 36 px displayed at 18 pt, system-tinted template |
| `tray-windows.png` / `tray-windows.ico` | Amber Windows tray; PNG at 32 px, ICO at 16–64 px |

The macOS bundle includes `Soundy.icns`; the menu bar PNG is embedded in the executable. Soundy currently supports macOS only. The Windows icons are ready for a future Windows implementation.

To regenerate all platform exports from `soundy.png` and `tray.svg` on macOS:

```sh
swift scripts/export-icons.swift
```

The master was generated using the built-in ImageGen tool. Prompt:

> One minimalist 3D application icon: a keyboard keycap with a recognizable stylized ear centered on its top face. Dark burnt-orange and amber glass, a thick rounded-square keycap, restrained refraction and polished bevel highlights. A bold smooth ear glyph, almost frontal view slightly from above, centered square composition, transparent background. No text, surrounding keyboard, extra symbols or watermark.
