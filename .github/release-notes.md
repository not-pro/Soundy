## Soundy 2.1.0

### English

- **Truly portable Windows build.** `Soundy.exe` is now a single file: all 240 WAV sounds are compressed into the EXE and only the active keyboard/mouse profile is decompressed into memory. No separate `Sounds/` folder is required.
- External `Sounds/` folder still works as an override for development or advanced users.

### Русский

- **По-настоящему portable-сборка Windows.** `Soundy.exe` теперь один файл: все 240 WAV-звуков сжаты внутрь EXE, а в память распаковываются только звуки активного профиля клавиатуры и мыши. Отдельная папка `Sounds/` больше не нужна.
- Внешняя папка `Sounds/` по-прежнему работает для отладки или опытных пользователей.

### 简体中文

- **真正便携的 Windows 版本。** `Soundy.exe` 现在是一个单独的文件：全部 240 个 WAV 音效被压缩在 EXE 内部，只有当前键盘和鼠标配置所需的音效会被解压到内存中。不再需要单独的 `Sounds/` 文件夹。
- 外部的 `Sounds/` 文件夹仍可作为覆盖，方便开发或高级用户使用。

Download the **PKG** for your Mac: **arm64** for Apple Silicon, **x86_64** for Intel. On Windows, use the portable **ZIP**. ZIP archives, signed update feeds, and SHA-256 checksums are included. Requires macOS 13 or later, or Windows 10 or later. The macOS app is self-signed, without Apple notarization.
