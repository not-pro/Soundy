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
    assert_eq!(
        MESSAGES.len(),
        Text::ALL.len(),
        "every Text key must exist in i18n.json"
    );
    for language in Language::ALL {
        assert_eq!(Language::from_code(language.code()), Some(language));
        for message in Text::ALL {
            let key = message.key();
            assert!(MESSAGES.contains_key(key), "i18n.json is missing {key}");
            assert!(
                !language.text(*message).trim().is_empty(),
                "{message:?} must not be empty"
            );
        }
        assert_ne!(language.mouse_name(0), language.mouse_name(1));
        assert!(language.mouse_name(15).ends_with(" C"));
    }
}
