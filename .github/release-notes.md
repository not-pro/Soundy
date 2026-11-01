## Soundy 1.10.0

### English

- **Every press sounds a little different.** Each sample is resampled a few percent up or down and its level moves by up to 1.5 dB, never repeating the previous variant, so long typing no longer sounds like one looped file. A held key still repeats nothing.
- **The right mouse button has its own click.** Every mouse profile ships two clicks; the second one now plays for the right button, while the primary and middle buttons keep the first.
- **The Bell typewriter answers key-up.** `t_bell_release` is the only release sample in the pack and now plays when a key is released, which is what that profile was missing.
- **The settings window follows the system appearance.** It uses native AppKit vibrancy again instead of the Liquid Glass view on macOS 26, matches light and dark mode, and no longer keeps the glass effect resident — the window costs several megabytes less memory while it is open. Text, icons, the accent colour, and the tray icon follow the same appearance.

### Русский

- **Каждое нажатие звучит немного иначе.** Сэмпл растягивается или сжимается на несколько процентов, громкость меняется до ±1,5 дБ, а предыдущий вариант не повторяется — длинный текст больше не звучит как зацикленный файл. Зажатая клавиша по-прежнему не повторяет звук.
- **У правой кнопки мыши свой клик.** В каждом профиле мыши два клика: второй теперь играет на правую кнопку, а основная и средняя остаются на первом.
- **Печатная машинка Bell отвечает на отпускание клавиши.** `t_bell_release` — единственный release-сэмпл в наборе, и теперь он звучит при отпускании клавиши: этого профилю не хватало.
- **Окно настроек следует теме системы.** Вместо Liquid Glass в macOS 26 снова используется нативная прозрачность AppKit: окно соответствует светлой и тёмной теме и больше не держит стеклянный эффект в памяти — при открытом окне расходуется на несколько мегабайт меньше. Текст, иконки, акцентный цвет и значок в строке меню следуют той же теме.

### 简体中文

- **每次按键都略有不同。** 每个采样会升高或降低几个百分点、音量变化最多 ±1.5 dB，并且不会重复上一个变体，长时间打字不再像在循环播放同一个文件。按住按键仍然不会重复发声。
- **鼠标右键有自己的点击声。** 每个鼠标配置都带有两个点击声：第二个现在用于右键，主键和中键继续使用第一个。
- **Bell 打字机配置会响应松开按键。** `t_bell_release` 是音效包中唯一的 release 采样，现在会在松开按键时播放，这正是该配置所缺少的。
- **设置窗口跟随系统外观。** 在 macOS 26 上不再使用 Liquid Glass，而是重新使用原生 AppKit 半透明效果，匹配浅色和深色模式，也不再常驻玻璃效果——窗口打开时内存占用减少数兆字节。文字、图标、强调色和菜单栏图标都遵循同一外观。

Download the **PKG** for your Mac: **arm64** for Apple Silicon, **x86_64** for Intel. On Windows, use the portable **ZIP**. ZIP archives, signed update feeds, and SHA-256 checksums are included. Requires macOS 13 or later, or Windows 10 or later. The macOS app is self-signed, without Apple notarization.
