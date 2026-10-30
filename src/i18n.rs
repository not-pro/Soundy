use objc2_foundation::{NSLocale, NSString, NSUserDefaults, ns_string};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(usize)]
pub enum Language {
    English,
    Russian,
    Chinese,
}

impl Language {
    pub const ALL: [Self; 3] = [Self::English, Self::Russian, Self::Chinese];

    pub fn code(self) -> &'static str {
        ["en", "ru", "zh-Hans"][self as usize]
    }

    pub fn name(self) -> &'static str {
        ["English", "Русский", "简体中文"][self as usize]
    }

    pub fn from_code(code: &str) -> Option<Self> {
        match code.split(['-', '_']).next()?.to_ascii_lowercase().as_str() {
            "en" => Some(Self::English),
            "ru" => Some(Self::Russian),
            "zh" => Some(Self::Chinese),
            _ => None,
        }
    }

    pub fn resolve(saved: Option<&str>, preferred: impl IntoIterator<Item = String>) -> Self {
        saved
            .and_then(Self::from_code)
            .or_else(|| {
                preferred
                    .into_iter()
                    .find_map(|code| Self::from_code(&code))
            })
            .unwrap_or(Self::English)
    }

    pub fn current() -> Self {
        Self::load(&NSUserDefaults::standardUserDefaults())
    }

    pub fn load(defaults: &NSUserDefaults) -> Self {
        let saved = defaults
            .stringForKey(ns_string!("language"))
            .map(|value| value.to_string());
        Self::resolve(
            saved.as_deref(),
            NSLocale::preferredLanguages().iter().map(|s| s.to_string()),
        )
    }

    pub fn save(self, defaults: &NSUserDefaults) {
        let code = NSString::from_str(self.code());
        // SAFETY: NSUserDefaults copies this immutable NSString property-list value.
        unsafe { defaults.setObject_forKey(Some(&code), ns_string!("language")) };
    }

    pub fn mouse_name(self, index: u8) -> String {
        match index {
            0 => self.text(Text::Off).into(),
            15 => format!("{} C", self.text(Text::Click)),
            n => format!("{} {n}", self.text(Text::Click)),
        }
    }
}

// Every message has all three translations; adding a message requires the full row.
macro_rules! messages {
    ($($key:ident => [$en:expr, $ru:expr, $zh:expr]),* $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq)]
        pub enum Text { $($key),* }

        impl Language {
            pub fn text(self, message: Text) -> &'static str {
                match message { $(Text::$key => [$en, $ru, $zh][self as usize]),* }
            }
        }

        #[cfg(test)]
        const ALL_MESSAGES: &[Text] = &[$(Text::$key),*];
    };
}

messages! {
    OpenSettings => ["Open Soundy…", "Открыть Soundy…", "打开 Soundy…"],
    Keyboard => ["Keyboard", "Клавиатура", "键盘"],
    Mouse => ["Mouse", "Мышь", "鼠标"],
    Preview => ["Preview sound", "Прослушать звук", "试听音效"],
    CloseWindow => ["Close window", "Закрыть окно", "关闭窗口"],
    About => ["About Soundy", "О Soundy", "关于 Soundy"],
    Version => ["Version", "Версия", "版本"],
    SoundEnabled => ["Sound enabled", "Звук включён", "音效已开启"],
    SoundMuted => ["Sound muted", "Звук выключен", "音效已静音"],
    AllowInput => ["Allow Input Monitoring…", "Разрешить мониторинг ввода…", "允许输入监控…"],
    RetryInput => ["Reconnect Input…", "Повторить подключение ввода…", "重新连接输入设备…"],
    KeyboardSound => ["Keyboard Sound", "Звук клавиатуры", "键盘音效"],
    MouseSound => ["Mouse Sound", "Звук мыши", "鼠标音效"],
    Mechanical => ["Mechanical", "Механические", "机械键盘"],
    Electronic => ["Electronic", "Электронные", "电子音效"],
    Fun => ["Fun", "Эффекты", "趣味音效"],
    Typewriters => ["Typewriters", "Печатные машинки", "打字机"],
    LaunchAtLogin => ["Launch at Login", "Запуск при входе", "登录时启动"],
    UpdateAvailable => ["Update Available", "Доступно обновление", "有可用更新"],
    ApproveLogin => ["Allow in Login Items…", "Разрешить в объектах входа…", "在登录项中允许…"],
    LoginFailed => ["Could Not Change Launch at Login", "Не удалось изменить автозапуск", "无法更改登录时启动设置"],
    Volume => ["Volume", "Громкость", "音量"],
    LanguageMenu => ["Language", "Язык", "语言"],
    Quit => ["Quit Soundy", "Завершить Soundy", "退出 Soundy"],
    Off => ["Off", "Выключен", "关闭"],
    Click => ["Click", "Клик", "点击"],
    Tooltip => ["Soundy — keyboard and mouse sounds", "Soundy — звуки клавиатуры и мыши", "Soundy — 键盘和鼠标音效"],
    InputUnavailable => ["Soundy — input monitoring unavailable", "Soundy — мониторинг ввода недоступен", "Soundy — 输入监控不可用"],
    StartFailed => ["Could Not Start Soundy", "Не удалось запустить Soundy", "无法启动 Soundy"],
    OK => ["OK", "ОК", "好"],
    InputPermissionRequired => ["Input Monitoring permission is required", "Требуется доступ к мониторингу ввода", "需要授予输入监控权限"],
    InputConnectionFailed => ["Could not connect the keyboard and mouse. Restart Soundy after granting access.", "Не удалось подключить клавиатуру и мышь. Перезапустите Soundy после выдачи доступа.", "无法连接键盘和鼠标。授予权限后，请重新启动 Soundy。"],
    InputSourceFailed => ["Could not create the keyboard event source", "Не удалось создать источник событий клавиатуры", "无法创建键盘事件源"],
    MainLoopUnavailable => ["The main run loop is unavailable", "Основной run loop недоступен", "主运行循环不可用"],
    MainThreadRequired => ["Soundy must run on the main thread", "Soundy должен запускаться в основном потоке", "Soundy 必须在主线程上运行"],
    AudioThreadStopped => ["The audio thread stopped during startup", "Аудиопоток завершился при запуске", "音频线程在启动时停止"],
    AudioOutputFailed => ["Could not start audio output", "Не удалось запустить аудиовыход", "无法启动音频输出"],
    PathUtf8Required => ["The sounds path must be UTF-8", "Путь к звукам должен быть UTF-8", "音效路径必须采用 UTF-8 编码"],
    InvalidAudioFormat => ["Invalid audio format or duration", "Недопустимый формат или длительность", "音频格式或时长无效"],
    WavTooLarge => ["WAV file is too large", "Слишком большой WAV", "WAV 文件过大"],
    AudioBufferFailed => ["Could not allocate an audio buffer", "Не удалось выделить аудиобуфер", "无法分配音频缓冲区"],
    IncompleteWav => ["Incomplete WAV file", "Неполный WAV", "WAV 文件不完整"],
    InvalidSoundFile => ["Invalid sound file", "Недопустимый звуковой файл", "音效文件无效"],
    SoundsPathRequired => ["--sounds-dir requires a WAV directory path", "--sounds-dir требует путь к WAV", "--sounds-dir 需要提供 WAV 目录路径"],
    UnknownArgument => ["Unknown argument", "Неизвестный аргумент", "未知参数"],
    Help => [
        "Soundy — keyboard and mouse sounds for macOS\n\n  --sounds-dir PATH  Custom WAV directory (bundled sounds by default)\n  --check            Validate 240 WAV files without audio or input monitoring\n  --preview          Play one keyboard sound and exit\n  --preview-mouse    Play one mouse click and exit\n  --version          Version\n",
        "Soundy — звуки клавиатуры и мыши для macOS\n\n  --sounds-dir PATH  Свой каталог WAV (по умолчанию встроенные звуки)\n  --check            Проверить 240 WAV без звука и мониторинга ввода\n  --preview          Воспроизвести один звук и завершить работу\n  --preview-mouse    Воспроизвести клик мыши и завершить работу\n  --version          Версия\n",
        "Soundy — macOS 键盘和鼠标音效\n\n  --sounds-dir PATH  自定义 WAV 目录（默认使用内置音效）\n  --check            检查 240 个 WAV 文件，不播放声音或监控输入\n  --preview          播放一个键盘音效后退出\n  --preview-mouse    播放一次鼠标点击音效后退出\n  --version          版本\n"
    ],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_language_overrides_system_preferences_with_english_fallback() {
        assert_eq!(
            Language::resolve(Some("ru"), ["zh-CN".into()]),
            Language::Russian
        );
        assert_eq!(
            Language::resolve(None, ["de-DE".into(), "zh-Hans-CN".into()]),
            Language::Chinese
        );
        assert_eq!(
            Language::resolve(Some("invalid"), ["en_GB".into()]),
            Language::English
        );
        assert_eq!(Language::resolve(None, ["fr-FR".into()]), Language::English);
        for language in Language::ALL {
            assert_eq!(Language::from_code(language.code()), Some(language));
            for message in ALL_MESSAGES {
                assert!(!language.text(*message).trim().is_empty());
            }
            assert_ne!(language.mouse_name(0), language.mouse_name(1));
            assert!(language.mouse_name(15).ends_with(" C"));
        }
    }
}
