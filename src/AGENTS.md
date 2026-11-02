# AGENTS.md — краткая памятка для агентов

Полное описание архитектуры и рецепты расширения: [ARCHITECTURE.md](ARCHITECTURE.md).
Читать его целиком перед первой нетривиальной правкой; здесь — только то, что нельзя
нарушать и не нужно перечитывать.

## Что это

Soundy — трей-приложение на Rust (macOS AppKit/AVFoundation, Windows Win32/rodio),
которое играет звуки клавиатуры и мыши. Три слоя: `core/` (Model), `view_model.rs`
(ViewModel), `ui/` + `ui_windows*.rs` (View). Обе платформы собираются из одного дерева
через `cfg` + `#[path]`.

## Жёсткие правила

1. VM (`view_model.rs`) не трогает виджеты, View (`ui*`) не содержит правил.
2. После каждой мутации VM View вызывает `refresh()` — уведомлений нет.
3. Звук читает состояние только из `Arc<Controls>`-атомиков; в аудио-поток — только
   `Command` и только `try_send`.
4. Имена WAV — только через `core/catalog.rs`; порядок `assets/sounds/manifest.sha256`
   (алфавитный) = индексы = порядок бандла Windows.
5. Пользовательский текст — `Text` + `assets/i18n.json` для `en`, `ru`, `zh-Hans`.
6. Счётчики и размеры в `--smoke-test` фиксированы: изменил меню/окно — обнови ожидания
   в том же коммите.
7. Платформенная развилка — `cfg` + `#[path]`. Никаких `trait Platform`/фабрик/слоя событий.
8. Не добавлять зависимость, если хватает std или уже подключённого crate.

## Куда что класть

| Задача | Файлы |
|---|---|
| Настройка (хранение) | `view_model.rs` (поле + метод + trait `Persistence`) и обе реализации ниже |
| Ключи настроек macOS | `view_model::mac::Persistence` (`NSUserDefaults`) |
| Ключи настроек Windows | `core/settings.rs` (`Config`) + `view_model::windows::Persistence` |
| Правило домена | `core/*`, отдельным модулем, с юнит-тестами |
| Влияние на звук | `Controls` в `core/audio.rs` **и** `core/audio_windows.rs` + чтение в воркере |
| Пункт меню macOS | селектор в `define_class! Delegate` + `refresh()` в `ui/mod.rs` |
| Пункт меню Windows | `MENU_*` + `App::activate` + `build_menu` + `MENU_ITEMS` в `ui_windows.rs` |
| Контрол настроек | `ui/settings.rs` (macOS) / `ui_windows_settings.rs` (Windows) |
| Новая строка | `core/i18n.rs` + `assets/i18n.json` |
| Новый звук | `assets/sounds/` + `manifest.sha256` + `KEYBOARDS` в `core/catalog.rs` |

Подробные пошаговые рецепты (включая чек-листы обновления счётчиков) — §9 ARCHITECTURE.md.

## Проверки (все обязательны перед пушем)

```sh
cargo fmt --all -- --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo test  --locked --offline -- --test-threads=1
cargo run   --locked --offline -- --check
cargo run   --locked --offline -- --smoke-test
```

`cargo test` — последовательно (`--test-threads=1`): macOS-тесты поднимают `AVAudioEngine`.
Тест `login_tests` помечен `#[ignore]` — он требует подписанного бандла.

## Известные грабли

* **Windows игнорирует `enabled`**: флаг сохраняется, но `audio_windows::Controls` его не
  имеет, и звук глушится только `volume = 0`.
* macOS-окно настроек берёт данные **из трей-меню**: пункт, которого нет в меню, в окно не
  попадёт.
* Windows-окно настроек находит `App` по цепочке `GWLP_USERDATA` (своё окно → owner → `App`).
* `main.rs` печатает захардкоженные числа каталога («240 WAV…») — править при изменении паков.
* `ponytail:`-комментарии — осознанные упрощения с известным потолком; менять вместе с
  комментарием.
