## Soundy 2.0.0

### English

- **Mute everything without losing your setup.** The new **Enabled** item turns all sounds off with the profiles, volume, and hotkeys kept; one click or a recorded shortcut brings them back.
- **Global hotkeys.** The **Shortcuts** menu records combinations for sound on/off, volume up/down, and toggling the current app's sound: click an action, press the keys, done — Esc cancels. Hotkeys keep working while sounds are muted.
- **Per-app profiles.** The **For <App>** menu pins a keyboard profile, a mouse sound, or complete silence to the frontmost application; everything without an override follows your global choice.
- **Play Through.** Restrict sounds to headphones or speakers. Soundy classifies the current output (Bluetooth and the headphone jack vs. USB, HDMI, and built-in speakers) and shows it in the menu; an unrecognized device never goes silent by mistake.
- **Stereo Key Position.** Optional: pans each key to its physical spot on the keyboard, left keys left and right keys right.
- **Lower memory use on macOS.** Sounds from a previously selected profile are released after 10 seconds, and the per-press pitch variants are freed as soon as they finish playing.
- **The settings window uses a plain theme background now** — light or dark, matching the system, with no translucent glass effects.
- Hotkeys, per-app profiles, and Play Through are macOS-only in this release; the Windows build keeps its existing tray features.

### Русский

- **Выключите звук, не теряя настройки.** Новый пункт **Включено** глушит все звуки, сохраняя профили, громкость и горячие клавиши; один клик или горячая клавиша возвращает всё обратно.
- **Горячие клавиши.** Меню **Горячие клавиши** записывает сочетания для звука вкл/выкл, громкости ± и переключения звука текущего приложения: нажмите на действие, затем клавиши — Esc отменяет. Горячие клавиши работают, даже когда звук выключен.
- **Профили для приложений.** Меню **Для: <приложение>** закрепляет за активным приложением профиль клавиатуры, звук мыши или полную тишину; всё остальное следует глобальным настройкам.
- **Воспроизводить через.** Ограничьте звук наушниками или динамиками. Soundy определяет текущее устройство (Bluetooth и разъём наушников против USB, HDMI и встроенных динамиков) и показывает его в меню; нераспознанное устройство не замолчит случайно.
- **Стереопозиция клавиш.** По желанию: панорамирует каждую клавишу на её физическое место — левые слева, правые справа.
- **Меньше памяти в macOS.** Звуки сменённого профиля освобождаются через 10 секунд, а варианты высоты освобождаются сразу после звучания.
- **Окно настроек теперь с обычным фоном темы** — светлое или тёмное по системе, без стеклянных полупрозрачных эффектов.
- Горячие клавиши, профили приложений и выбор устройства пока только в версии для macOS; сборка для Windows сохраняет прежние возможности трея.

### 简体中文

- **静音但不丢设置。** 新增的**已启用**选项会关闭所有声音，同时保留配置、音量和快捷键；单击一次或按下快捷键即可恢复。
- **全局快捷键。** **快捷键**菜单可为声音开/关、音量加/减和切换当前应用声音录制组合键：点击动作后按下按键即可，Esc 取消。即使声音已关闭，快捷键仍然有效。
- **按应用配置。** **当前应用的音效**菜单可为当前应用指定键盘配置、鼠标音效或完全静音；未设置覆盖的应用继续使用全局配置。
- **播放设备。** 可将声音限制在耳机或扬声器。Soundy 会识别当前输出设备（蓝牙耳机和耳机插孔 vs. USB、HDMI 和内置扬声器）并显示在菜单中；无法识别的设备不会被误静音。
- **按键立体声定位。** 可选：按物理位置为每个按键声像定位，左侧按键偏左、右侧按键偏右。
- **macOS 内存占用更低。** 切换前的旧音效 10 秒后释放，每次按键的音高变体在播放结束后立即释放。
- **设置窗口现在使用纯主题背景**——跟随系统的浅色或深色，不再有半透明玻璃效果。
- 快捷键、按应用配置和播放设备选择目前仅支持 macOS；Windows 版本保留原有的托盘功能。

Download the **PKG** for your Mac: **arm64** for Apple Silicon, **x86_64** for Intel. On Windows, use the portable **ZIP**. ZIP archives, signed update feeds, and SHA-256 checksums are included. Requires macOS 13 or later, or Windows 10 or later. The macOS app is self-signed, without Apple notarization.
