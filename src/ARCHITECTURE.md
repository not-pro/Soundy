# Архитектура Soundy: MVVM и правила расширения

Документ в двух частях: **как код устроен сейчас** (2.2.0) и **как добавлять функционал**, не ломая слои.
Короткая версия для агентов — [AGENTS.md](AGENTS.md).

Исходники — Rust 2024, без GUI-фреймворка: на macOS AppKit/AVFoundation, на Windows Win32/rodio.
Платформенная развилка сделана не абстракцией, а `cfg` + `#[path]` — это осознанно, см. §8.

---

## 1. Слои

| Слой | Файлы | Отвечает за | Не имеет права |
|---|---|---|---|
| Model | `core/*` | звук, ввод, каталог, настройки-на-диске, i18n, маршруты | знать про View/ViewModel, строить UI, читать состояние UI |
| ViewModel | `view_model.rs` | состояние приложения, персистентность, применение профилей, политика хоткеев | вызывать AppKit/Win32-виджеты, создавать окна/меню, крутить message loop |
| View | `ui/*`, `ui_windows*.rs` | отрисовка, нативные контролы, селекторы/wndproc, владение окнами | хранить бизнес-состояние, писать в `Controls` напрямую |

```
┌──────────────────────── main thread ────────────────────────┐      ┌──── audio thread ────┐
│  View (ui/mod.rs, ui_windows.rs)                            │      │  Worker              │
│     │ вызывает методы VM            refresh() ▲             │      │   Player / Audio     │
│     ▼                                      │                │      │                      │
│  ViewModel (view_model.rs) ────────────────┘                │      │                      │
│     ├─ Persistence ──▶ NSUserDefaults / config.json         │      │                      │
│     └─ Arc<Controls> ── атомики ────────────────────────────┼─────▶│  читает атомики      │
│                                                             │      │                      │
│     Command ── mpsc SyncSender(32) ─────────────────────────┼─────▶│  очередь команд      │
└─────────────────────────────────────────────────────────────┘      └──────────────────────┘
        ▲ InputTap / InputHook ── try_send(Command::Key { at })
```

### Файлы

```
src/
  main.rs                  аргументы CLI, выбор источника звуков, запуск ui::run
  consts/mod.rs            ссылки из assets/config.json (GitHub, Telegram)
  view_model.rs            ViewModel, trait Persistence, mac::/windows:: реализации
  ui/mod.rs                macOS: Delegate, трей-меню, статус-иконка, жизненный цикл InputTap
  ui/settings.rs           macOS: окно 240×240, popover About, отрисовка на NSView
  ui_windows.rs            Windows: App, трей, wndproc, message loop, хуки
  ui_windows_settings.rs   Windows: окно настроек на Win32-контролах
  core/mod.rs              cfg-выбор платформенных реализаций модулей
  core/audio.rs            macOS: AVAudioEngine, кэш сэмплов, питч/пан, воркер
  core/audio_windows.rs    Windows: rodio, декод на каждое нажатие, воркер
  core/bundle.rs           Windows: WAV, сжатые в EXE на этапе build.rs
  core/catalog.rs          единственный реестр звуков: профиль → индекс сэмпла
  core/input.rs            macOS: CGEventTap (нажатие/отпускание/мышь/хоткеи)
  core/input_windows.rs    Windows: WH_KEYBOARD_LL + WH_MOUSE_LL
  core/hotkeys.rs          модель шорткатов и их хранение (фича macOS)
  core/perapp.rs           overrides по bundle id (фича macOS)
  core/routes.rs           CoreAudio: класс устройства вывода + листенер (macOS)
  core/pan.rs              стерео-позиция клавиши по ANSI-раскладке (macOS)
  core/variation.rs        питч и уровень каждого нажатия (обе платформы)
  core/i18n.rs             Language + Text; тексты в assets/i18n.json
  core/login.rs            macOS: SMAppService
  core/login_windows.rs    Windows: HKCU\...\Run
  core/settings.rs         Windows: %APPDATA%\Soundy\config.json
  core/update.rs           macOS: Sparkle
```

---

## 2. Точка входа и платформенная развилка

`main.rs` разбирает аргументы и уходит в один из режимов:

| Аргумент | Что делает |
|---|---|
| `--check` | `catalog::validate` + декодирование всех WAV, без аудио и хуков |
| `--preview` / `--preview-mouse` | поднимает воркер, играет один превью-сэмпл, выходит |
| `--smoke-test` | запускает UI, проверяет меню и жизненный цикл окна, выходит |
| `--sounds-dir <path>` | источник звуков — папка вместо встроенного бандла (Windows) |

Один и тот же модуль `ui` подключается к разным файлам:

```rust
#[cfg(target_os = "macos")] mod ui;
#[cfg(windows)] #[path = "ui_windows.rs"] mod ui;
```

Тот же приём в `core/mod.rs` для `audio`, `input`, `login`. Поэтому сигнатуры `ui::run` и
`audio::Worker::start` на платформах разные, а `main.rs` склеивает их через `#[cfg]`-блоки.
**Новый платформенный модуль добавляется туда же, в `core/mod.rs`, а не через `trait Platform`.**

---

## 3. Model (`core/`)

Правила слоя:

* никаких типов AppKit/Win32 (кроме самих платформенных модулей, которые они и реализуют);
* ошибки — `Result<_, String>` с текстом из `Language::text(Text::…)`, чтобы его можно было
  показать в меню/алерте как есть;
* вся настройка поведения — через входные аргументы, а не через глобальные синглтоны;
* состояние, которое читает аудио-поток, лежит в `Arc<Controls>` и только в атомиках.

### Каталог — единственный источник правды

`core/catalog.rs` — не список профилей, а **маршрутизация**: имя WAV → индекс в
`assets/sounds/manifest.sha256` → сэмпл. Порядок строк манифеста = индекс = порядок в
сжатом бандле Windows, поэтому манифест отсортирован по имени, а `build.rs` сортирует так же
(тест `bundle` это проверяет).

* `Profile(u8)` — 40 клавиатурных паков; префикс имени задаёт категорию (`m_`, `e_`, `fun_`, `t_`).
* `Profile::sample_index(keycode)` — спец-клавиши (`space` 49, `enter` 36/76, `delete` 51/117)
  берут одноимённый файл, остальные — `keyN` по модулю количества.
* `Profile::release_index()` — только `t_bell` везёт `_release.wav`.
* `MouseProfile(u8)` — 0 = выключено, 1..=14 → `mouse_N_1/2.wav`, 15 → `mouse_c_1/2.wav`;
  правая кнопка = файл `_2`.

Всё остальное (`audio`, `bundle`) адресует звуки **только** через эти функции и `files()`.

### Аудио

`Worker::start` поднимает поток `soundy-audio` и возвращает `Worker { sender, thread }`.
Поток владеет движком, на каждой команде:

1. читает атомики (`profile`, `mouse_profile`, `volume`, на macOS ещё `enabled`,
   `keyboard_off`, `stereo_pan`, `route_allows`);
2. при смене профилей перекладывает кэш (`select_profiles`: нужные сэмплы держит, ненужные
   помечает на удаление через 10 с);
3. отбрасывает команду старше **100 мс** — очередь не имеет права копить задержку;
4. выбирает вариант питча/уровня (`core/variation.rs`) и играет.

macOS: `AVAudioEngine`, лимитер, пул из 6 голосов на PCM-формат, `resample` для питча,
`pan` для стерео-позиции, TTL кэша 10 с / свип 1 с.
Windows: `rodio`, один декод на нажатие, без кэша сэмплов; бандл-кэш (`core/bundle.rs`)
держит только активные профили в памяти.

`Command` — единственный канал в аудио: `Key`, `KeyUp`, `Mouse`, `Preview`, `PreviewMouse`,
`ProfilesChanged` (только macOS, прогрев), `ResetAudio`, `Stop`. Отправка всегда `try_send`:
полная очередь роняет звук, но не ввод.

### Ввод

* macOS (`core/input.rs`): listen-only `CGEventTap` в главном run loop; фильтрует автоповтор,
  считает нажатие/отпускание модификаторов по сторонам, различает правую кнопку мыши,
  и **до** отправки звука прогоняет `KeyDown` через политику хоткеев
  (`hotkeys::State::matched_by`, запись комбинации, Escape-отмена).
* Windows (`core/input_windows.rs`): `WH_KEYBOARD_LL` + `WH_MOUSE_LL` в потоке сообщений;
  отбрасывает инжектированный ввод (`LLKHF_INJECTED`), распознаёт автоповтор через `Pressed[256]`,
  и переводит VK в macOS-коды каталога (`keycode()`), уводя коллизии в `0x100 + vk`.

### Политика, а не UI

`core/hotkeys.rs`, `core/perapp.rs`, `core/routes.rs`, `core/pan.rs` — чистые правила
(что с чем совпадает, как резолвится override, пускает ли режим маршрута на это устройство).
Ни один из них не знает про View, поэтому все они покрыты юнит-тестами без GUI.
`routes::Mode::allows` намеренно не глушит звук при `DeviceClass::Unknown`: тишина из-за
ошибки классификации хуже лишнего звука.

### i18n

`Text` — enum ключей, тексты — `assets/i18n.json` (`en`, `ru`, `zh-Hans`).
`Language::text()` отдаёт ключ, если перевода нет, — не паникует.
Правило: ни одна строка, видимая пользователю, не пишется в коде буквально (кроме имён паков —
они английские по дизайну).

---

## 4. ViewModel (`view_model.rs`)

`ViewModel` — обычная структура с interior mutability, **не** `Send`/`Sync` и живёт только
на главном потоке. Хранилища по типу данных:

| Тип | Поля |
|---|---|
| `Cell<Copy>` | `language`, `global`, `volume`, `enabled`, `profile`, `mouse_profile`, `settings_open`, `about_open`, `update_probe_finished`, `stereo_pan`, `route_mode`, `output_class` |
| `RefCell` | `worker`, `persistence`, `overrides`, `frontmost`, `update_version`, `input_status` |
| `Arc<Mutex<…>>` | `hotkeys` (её читает input-колбэк) |
| `Arc<Controls>` | атомики, которые читает аудио-поток |

### Persistence

`trait Persistence` — единственная точка расхождения хранения:

* `view_model::mac::Persistence` — `NSUserDefaults`, ключи `profile`, `mouseProfile`, `volume`,
  `enabled`, `language`, `launchAtLogin`, `stereoKeyPosition`, `audioRoute`, `perApp`, `hotkeys`;
* `view_model::windows::Persistence` — `Config` (`core/settings.rs`), один JSON-файл,
  перезаписывается целиком на каждый `save_*`.

Геттеры читаются один раз в `ViewModel::new`; сеттеры вызываются **в каждом** мутирующем
методе VM сразу после изменения состояния. Платформенные методы (`stereo_pan`, `overrides`,
`hotkeys`, `route_mode`, `save_*`) имеют дефолтные реализации, поэтому Windows-реализация их
не пишет и компилируется без `cfg`-шума.

### Нет слоя уведомлений

Это важнейшее свойство архитектуры: **VM никого не уведомляет**. Контракт такой:

```rust
self.ivars().view_model.select_profile(profile);  // мутация
self.refresh();                                   // полная перерисовка
```

`refresh()` в каждой платформе перестраивает представление с нуля. Поэтому:

* нельзя кэшировать производные данные в View между вызовами;
* нельзя полагаться на инкрементальные обновления (единственное исключение — `sync_volume`
  на macOS, чтобы слайдер не прыгал во время drag);
* любой новый метод VM, меняющий отображаемое состояние, обязан вызываться вместе с `refresh()`
  на стороне View.

### Применение профилей

`apply_profiles()` — единственное место, где решается, что реально звучит:

```
perapp::resolve(overrides, frontmost, global_keyboard, global_mouse) -> (Option<u8>, u8)
```

`None` по клавиатуре означает «тихо для этого приложения» (override `keyboard: -1`) и
включает `controls.keyboard_off` на macOS. Отсюда следствие, о котором легко забыть:
**`select_profile` не меняет звук, если для активного приложения есть override** — глобальный
выбор сохраняется, но применяется только там, где override нет.

`set_override(keyboard, mouse)` подставляет глобальное значение для незаданной половины,
`toggle_app_sound` пишет `{keyboard: -1, mouse: 0}` или удаляет запись.

### Политика хоткеев

VM владеет только состоянием записи и раскладкой слота на действие:
`start_recording_hotkey`, `record_hotkey` (забирает комбинацию у другого действия),
`reset_hotkeys`, `handle_hotkey`, `shortcut_title`. Сам перехват — в `core/input.rs`,
отрисовка — в `refresh()`.

### Чего VM не делает

Не создаёт окна, не показывает `NSAlert`/`MessageBox`, не строит меню, не запускает таймеры,
не трогает `CGEventTap`/хуки. Всё это — View, потому что требует главного потока платформы.

---

## 5. Views

### macOS (`ui/mod.rs` + `ui/settings.rs`)

`Delegate` — наследник `NSObject` с `ivars: State { view_model, window, about, tap, status,
menu, updater, route_listener }`. Все действия — ObjC-селекторы внутри `define_class!`;
View не хранит состояние вне `State`.

* `refresh()` — единственная точка правды для UI: пересобирает меню целиком, при
  необходимости поднимает/роняет `InputTap`, обновляет tooltip и иконку, вызывает
  `settings::populate` и `about`.
* **Меню — источник данных для окна настроек.** `settings::populate` копирует submenu из
  трей-меню, снимает с пунктов target/action и превращает их в `NSPopUpButton`; кнопки
  настроек вызывают `settingsAction:` с `tag = index пункта в меню`. Popover About так же
  берёт из меню `approveLogin:`/`installUpdate:`/Quit. Следствие: всё, что должно появиться
  в настройках, обязано существовать в меню.
* Окно: `GlassWindow` с `setReleasedWhenClosed(false)`, скрытыми стандартными кнопками,
  `windowWillClose:` снимает владение и делает `autorelease`; пин `NSPopover` живёт в `State`.
* События: `NSWorkspace` (активация приложения → `set_frontmost_app`, wake → `restart_audio`),
  CoreAudio-листенер (`output_device_changed` → `performSelectorOnMainThread:routeChanged`),
  Sparkle-делегат (`update_found`, `update_probe_finished`).
* `--smoke-test` проходит по меню и окну и проверяет **точные размеры и количества** — это
  регрессия на всю вёрстку (см. §11).

### Windows (`ui_windows.rs` + `ui_windows_settings.rs`)

* `App { hwnd, view_model, hook, tray }` живёт на стеке `run()` и кладётся в
  `GWLP_USERDATA` скрытого окна; `wndproc` достаёт его оттуда и вызывает `show_menu`.
* Меню строится на каждый показ (`build_menu`) и уничтожается сразу после
  `TrackPopupMenu`; команды приходят числовыми `MENU_*`-ID в `App::activate`.
* Окно настроек находит `App` по цепочке `settings HWND → GWLP_USERDATA (= owner HWND) →
  GWLP_USERDATA (= *mut App)`. Любой новый контрол должен создать окно тем же `show()`,
  иначе `app_from_owner` вернёт `None` и обработка молча ничего не сделает.
* Сообщения: `WM_POWERBROADCAST` (resume → `Command::ResetAudio`), `WM_DESTROY` (снять иконку,
  `PostQuitMessage`).

### Общие правила для View

1. Виджет ничего не решает — он зовёт метод VM и делает `refresh()`.
2. Текст — только через `Text`.
3. Никаких глобальных переменных, кроме тех, что платформа навязывает (хуки в `ui_windows.rs`,
   lock в `ui/mod.rs`).
4. Владение ресурсом: пока View его держит — он жив; освобождать в обработчике закрытия, а не
   в середине callback'а.

---

## 6. Потоки и владение

| Поток | Что живёт здесь | Как общается |
|---|---|---|
| главный (main run loop / message loop) | ViewModel, все View, `InputTap`/`InputHook` колбэки, Sparkle-делегат | напрямую |
| `soundy-audio` | движок, кэш сэмплов, очередь `Command` | читает `Controls`, принимает `Command` |
| CoreAudio listener (macOS) | системный колбэк смены устройства | `performSelectorOnMainThread` → `routeChanged` |

Правила:

* `ViewModel` и все UI-объекты — только главный поток (`MainThreadOnly` в AppKit, отсутствие
  `Send` в VM это гарантирует);
* атомики в `Controls` пишет только главный поток, читает только аудио-поток, порядок
  `Relaxed` достаточен (значение — не инвариант, а «текущий выбор»);
* в аудио-поток нельзя отправлять блокирующе: только `try_send`, переполнение = потерянный
  звук;
* из аудио-потока нельзя дёргать UI. Если нужно (например, сброс после сна) — только через
  `Command`.

---

## 7. Сквозные сценарии

**Нажатие клавиши (macOS).**
`CGEventTap` → `hotkeys::matched_by` (если совпало — селектор на главный поток и `return`,
звука нет) → `try_send(Command::Key { keycode, at })` → воркер: не старше 100 мс? → `enabled` &&
`route_allows` && !`keyboard_off`? → `profile.sample_index(keycode)` → кэш/загрузка →
`variation::pick` + `level` → `AVAudioPlayerNode.scheduleBuffer` (с `Interrupts`, если голос занят).

**Нажатие клавиши (Windows).**
`WH_KEYBOARD_LL` → фильтр `LLKHF_INJECTED` → `Pressed::down` (первое нажатие?) →
`keycode(vk)` → `try_send` → воркер: `profile.sample_index` → `Sounds::get` (файл или
бандл-кэш) → `Decoder` + `speed(ratio)` + громкость → `Player::connect_new`.

**Изменение настройки.**
View → метод VM (clamp → запись в `Cell`/`RefCell` → `save_*` → атомики, если звук зависит) →
`refresh()` → меню/окно перерисованы с нуля. Превью играется отдельным `Command::Preview`.

**Смена активного приложения (macOS).**
`NSWorkspaceDidActivateApplicationNotification` → `set_frontmost_app` (сравнение с текущим,
чтобы не дёргать зря) → `apply_profiles()` → `perapp::resolve` → атомики → `refresh()`.

**Смена устройства вывода (macOS).**
CoreAudio колбэк → `routeChanged` на главном потоке → `set_output_class` (пересчёт
`route_allows`) + `restart_audio` (`Command::ResetAudio`) → воркер пересоздаёт движок с
5 попытками и заново прогревает кэш профилей.

**Сон/пробуждение.**
macOS: `NSWorkspaceDidWakeNotification` → `restart_audio`. Windows: `WM_POWERBROADCAST` →
`Command::ResetAudio`.

**Проверка обновлений (macOS).**
`Updater::start` в `applicationDidFinishLaunching` → `checkForUpdateInformation` →
`update_found` кладёт версию в VM → `refresh()` рисует пункт «Update Available» →
`installUpdate:` → Sparkle `checkForUpdates:`.

---

## 8. Инварианты (нарушение = баг архитектуры)

1. **VM не знает виджетов.** Ни одного `NSView`/`HWND` в `view_model.rs`.
2. **View не знает правил.** Никакой арифметики профилей/маршрутов/override'ов в UI.
3. **Одно состояние — одно место.** Значение живёт в VM; в меню/окне — только его отражение.
4. **Любая мутация заканчивается `refresh()`** (на macOS `refresh` ещё и владеет `InputTap`).
5. **Аудио-состояние идёт только через `Controls`-атомики**, команды — только через `Command`.
6. **Звуки адресуются только через `catalog`.** Ни один модуль не строит имя файла сам.
7. **Порядок `manifest.sha256` = индексы = порядок бандла Windows.** Манифест сортирован.
8. **Пользовательский текст — через `Text` + `assets/i18n.json` во всех трёх языках.**
9. **Строки каталога и формы меню зафиксированы smoke-тестами** — их надо обновлять вместе с
   изменением, а не «потом».
10. **Платформенная развилка — `cfg` + `#[path]`**, без `trait Platform` и фабрик: платформы
    расходятся слишком сильно, абстракция станет вторым набором условий.

---

## 9. Как добавлять функционал

Ниже — рабочие рецепты. Каждый заканчивается проверками из §11.

### A. Новая настройка, не влияющая на звук

1. `view_model.rs`: поле в `ViewModel` (`Cell` для `Copy`, `RefCell` иначе), геттер,
   мутирующий метод: `set_*` → clamp → запись → `persistence.borrow().save_*`.
2. `trait Persistence`: геттер и `save_*`. Для платформенной настройки — дефолтные реализации.
3. `view_model::mac::Persistence`: ключ в `NSUserDefaults`; `view_model::windows::Persistence`:
   поле в `Config` + `from_json`/`to_json`. Не забыть `Config::default()`.
4. `core/i18n.rs`: ключ в `text_keys!`; `assets/i18n.json`: `en`, `ru`, `zh-Hans`.
5. View: пункт меню и/или контрол (рецепт C), затем `refresh()`.
6. Smoke-тест: обновить ожидаемые количества и размеры (§11).
7. Тест: если есть логика (резолв, сериализация) — юнит-тест в модуле рядом с кодом.

### B. Настройка, влияющая на воспроизведение

Дополнительно к A:

1. Поле в **обоих** `Controls`: `core/audio.rs` и `core/audio_windows.rs` (одинаковое имя).
2. Запись атомика в методе VM (под `#[cfg]`, если платформа не умеет).
3. Чтение атомика в воркере — в ветке команды, а не один раз при старте.
4. Если поведение платформенное (как `stereo_pan`) — на второй платформе оставить значение по
   умолчанию и задокументировать асимметрию в §10.

### C. Новый пункт меню / действие

macOS:

1. `#[unsafe(method(name:))] fn name(&self, sender: &NSMenuItem)` внутри `define_class! Delegate`.
2. В `refresh()` добавить пункт: `self.item(menu, title, Some(sel!(name:)), tag, checked)`.
3. Если нужен подраздел — `self.submenu(menu, title)`.
4. Если пункт должен быть доступен из окна настроек — дать ему `tag = index в меню` и
   обработать через `settingsAction:`; пункт обязан быть в меню.
5. Обновить `smoke_finish`: `menu.numberOfItems()`, количество в подменю, теги.

Windows:

1. Константа `MENU_*` (диапазоны: 100 клавиатура, 200 мышь, 300 громкость, 400 язык, 500 прочее).
2. Ветка в `App::activate`.
3. Пункт в `build_menu`.
4. Обновить `MENU_ITEMS` и `App::check_menu` (там сверяются индексы подменю и количества).

### D. Новый звуковой пак или звук

1. Положить WAV в `assets/sounds/` по схеме `<prefix>_<role>.wav`, где role —
   `keyN` (рядовые), `space`, `enter`, `delete`, `release`, либо пусто (`.wav` = базовый сэмпл).
2. Обновить `assets/sounds/manifest.sha256` — **строки отсортированы по имени файла**,
   формат `<sha256>  <name>` (два пробела).
3. `core/catalog.rs`: запись в `KEYBOARDS`, префикс задаёт категорию (`m_`, `e_`, `fun_`, `t_`).
4. Синхронизировать числа: `core/catalog_tests.rs` (240 файлов, 14/6/11/9 профилей),
   smoke-тест macOS (`numberOfItems() == 47`, количества по категориям), строка
   `OK: 240 WAV decoded…` в `main.rs`, раздел README.
5. `cargo run -- --check` (декодирует все WAV) и полный `cargo test` (bundle-тест сверяет
   порядок бандла с манифестом).

### E. Новая строка интерфейса

`Text` в `core/i18n.rs` + запись в `assets/i18n.json` для всех трёх языков.
`i18n_tests` упадёт, если ключа нет в JSON, строка пустая или лишний ключ в JSON.

### F. Новая платформенная фича (пример: routes/hotkeys/perapp)

1. Модуль в `core/`; в `core/mod.rs` — `#[cfg(target_os = "macos")] pub mod x;`.
   Если модуль компилируется на обеих платформах, но нужен одной — `#[cfg_attr(not(...),
   allow(dead_code))]` с комментарием (так сделано для `hotkeys`, `pan`, `perapp`).
2. API в VM: методы под `#[cfg]`, на «чужой» платформе — либо нет метода, либо дефолт в
   `Persistence`.
3. View: пункт меню/контрол только на платформе, которая умеет (в меню macOS и в
   Windows-меню это разные места, но состояние — из одного VM).
4. Документировать асимметрию в §10 и README (в README есть отдельный абзац про macOS-only
   возможности).

### G. Новый `Command` в аудио-воркер

1. Вариант в `Command` **обоих** `audio`-модулей (или только в нужном, если фича
   платформенная, — тогда вызывающая сторона тоже под `#[cfg]`).
2. Ветка в `match command` воркера; помнить про проверку `at.elapsed() < 100 ms` для
   ввода и про то, что при `ResetAudio`/`ProfilesChanged` кэш надо перегреть.
3. Отправка из VM: `worker.sender.try_send(...)` — никогда не `send`.
4. Если команда меняет звук, она должна читать атомики, а не данные в момент отправки.

### H. Что не надо делать

* не заводить `trait View`/`trait Platform` — представлений ровно два, они не взаимозаменяемы;
* не добавлять слой событий/подписок (см. §4): `refresh()` дешевле и уже проверен;
* не кэшировать состояние в View;
* не класть бизнес-логику в `settings.rs`/`ui_windows_settings.rs`;
* не расширять `Persistence` платформенным методом без дефолтной реализации.

---

## 10. Известные асимметрии и долги

Проверено по коду, не «на глаз»:

* **Windows: `enabled` не глушит звук.** `toggle_enabled` сохраняет флаг и на macOS пишет
  `Controls::enabled`, но `audio_windows::Controls` такого поля не имеет, а воркер его не
  читает. Фактически на Windows тишина достигается только `volume = 0`. Это кандидат на
  первый фикс для агента, берущегося за Windows (рецепт B).
* **Windows не имеет**: глобальных хоткеев, per-app overrides, маршрутов вывода, стерео-позиции,
  Sparkle-обновлений. Модули `hotkeys`/`perapp`/`pan` компилируются, но не подключены
  (`allow(dead_code)`), VM-методы для них на Windows не вызываются.
* **Окно настроек Windows**: подписи контролов создаются по-английски литералами
  (`"Enabled"`, `"Keyboard:"`, …) и не перерисовываются при смене языка — в отличие от
  macOS, где всё через `Text`.
* **Окно настроек Windows** находит `App` через двойной `GWLP_USERDATA`; это неявный контракт
  владельца окна.
* **Окно настроек macOS** пересобирает content view на каждый `refresh()`, поэтому во время
  drag слайдера обновляется только `sync_volume`, без пересборки.
* **macOS-меню — источник данных для настроек**: popup'ы копируются из submenu. Новый
  контрол настроек, которого нет в меню, потребует ручного источника данных.
* **`main.rs` печатает захардкоженные числа** («240 WAV decoded, 40 keyboard profiles,
  15 mouse pairs»). При изменении каталога строку надо править.
* **`ponytail:`-маркеры** — осознанные упрощения с известным потолком (15 мс гейт на файл,
  6 голосов на PCM-формат). Менять вместе с комментарием, а не молча.
* `Profile::sample_index` для части паков использует модульную маршрутизацию, подтверждённую
  только для Tactile/Thocky/Topre; остальное — по ролям файлов (см. комментарий в `catalog.rs`).

---

## 11. Проверки

```sh
cargo fmt --all -- --check
cargo clippy --locked --offline --all-targets -- -D warnings
cargo test  --locked --offline -- --test-threads=1
cargo run   --locked --offline -- --check
cargo run   --locked --offline -- --smoke-test
```

* `cargo test` — 19 тестов + 1 `#[ignore]` (`login_tests`: запускается только из подписанного
  бандла с уникальным bundle id). Последовательно (`--test-threads=1`), потому что два теста
  поднимают `AVAudioEngine`, а параллельная инициализация на Intel-раннерах нестабильна.
* `--check` — `catalog::validate` (все 240 файлов на месте и не больше 16 МБ) + декодирование
  каждого WAV, без открытия аудиоустройства и без хуков.
* `--smoke-test` — прогон всего UI без ввода:
  macOS проверяет 16 пунктов меню (+1 при `RequiresApproval`), 47 пунктов в Keyboard Sound
  (счётчики категорий), 16 в Mouse Sound, 3 языка, окно ровно 240×240 с контролами внутри
  границ, popover 180×112, сохранение настроек, повторное открытие того же окна и
  освобождение после закрытия;
  Windows — форму трей-меню (`MENU_ITEMS`, индексы подменю, 47/16/6/3) без аудиоустройства.
  **Любое изменение меню или окна ломает смоук, пока не обновлены ожидания.**
* CI (`.github/workflows/release.yml`) гоняет fmt, clippy, test и `--smoke-test` на macOS
  (arm64 + x86_64) и на Windows; `--check` — после сборки бандла.

## 12. Чек-лист перед коммитом

- [ ] Нет UI-типов в VM и правил в View.
- [ ] Новая настройка: поле VM + `Persistence` (обе реализации) + `Text` + 3 языка.
- [ ] Новое влияние на звук: поле в **обоих** `Controls` + чтение в воркере.
- [ ] Каждая мутация во View сопровождается `refresh()`.
- [ ] Звуки адресуются через `catalog`, имена файлов не собираются вручную.
- [ ] Smoke-тесты и `main.rs`-сообщение синхронизированы с каталогом/меню.
- [ ] `fmt`, `clippy -D warnings`, `test`, `--check` зелёные; платформа, которую нет
      возможности собрать локально, проверена хотя бы `cfg`-чтением.
