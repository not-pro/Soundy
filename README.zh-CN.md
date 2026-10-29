[English](README.md) | [Русский](README.ru.md) | **简体中文**

<p align="center">
  <img src="assets/icons/soundy.png" width="160" height="160" alt="Soundy — 琥珀色玻璃键帽上的耳朵">
</p>

# Soundy

Soundy 为 macOS 的键盘和鼠标添加音效。机械键盘、打字机、电子提示音和趣味音效，让日常使用电脑的过程更加愉悦。

随心选择喜欢的声音：**40 种键盘音效和 15 种鼠标音效**。全部 240 个声音文件均已内置，无需额外下载。

Soundy 常驻菜单栏。你可以分别选择键盘和鼠标音效、试听声音、调节音量或暂停播放。设置会在退出后保留。你也可以关闭鼠标音效，同时保留键盘音效。

无需账号、无需订阅，也没有使用时限。所有功能均在本地运行。

## 开始使用

1. 从[最新版本](https://github.com/n-bautin/Soundy/releases/latest)下载 `.pkg` 安装包：Apple Silicon 芯片的 Mac 请选择 `arm64`，Intel 芯片的 Mac 请选择 `x86_64`。
2. 打开安装程序，依次点击**“继续” → “安装”**，然后点击**“关闭”**。从“应用程序”启动 Soundy。你也可以下载 ZIP 压缩包手动安装。
3. 在 Soundy 菜单中点击**“允许输入监控…”**，然后在 macOS 设置中授予权限。如果系统提示，请重新启动 Soundy。
4. 选择**“键盘音效”**和**“鼠标音效”**，调节音量，然后开始打字。

需要 **macOS 13 或更高版本**。

在受保护的密码输入框中，或启用“安全键盘输入”（Secure Keyboard Entry）时，键盘音效不可用：macOS 会阻止其他应用接收这些按键事件。退出安全输入状态后，音效会恢复。详情请参阅 [Apple — Secure Event Input](https://developer.apple.com/library/archive/technotes/tn2150/_index.html)。

## 语言

Soundy 支持**英语、俄语和简体中文**。首次启动时，应用会按 macOS 语言偏好的顺序选择第一个受支持的语言；如果没有匹配项，则使用英语。

在菜单栏中打开 **Language / Язык / 语言**，选择 **English**、**Русский** 或 **简体中文**。语言会立即切换，并在下次启动时保留。音效包名称仍使用英语。

## 构建

构建需要 Rust 和 macOS 开发者工具。原生安装包使用系统自带的 `pkgbuild` 和 `productbuild` 工具生成：

```sh
cargo fetch --locked
sh scripts/package.sh
```

生成的 PKG、ZIP 和校验和文件位于 `dist/` 目录。安装程序页面由 `scripts/installer-pages.swift` 生成，配置位于 `macos/installer/`。如需重新生成图标，请运行 `swift scripts/export-icons.swift`。
