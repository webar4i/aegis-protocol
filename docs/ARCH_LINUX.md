# Aegis Client на Arch Linux

Это приложение для разработки и проверки интерфейса: **Ionic React в окне
Tauri 2**. Capacitor остаётся оболочкой Android/iOS. Текущий GUI не связан
с CLI echo-прототипом, не создаёт TUN и не защищает интернет-трафик.

## Установка инструментов

Команды ниже предназначены для терминала на вашем Arch, под обычным
пользователем. Нужен графический сеанс Wayland или X11. Перед установкой
пакетов выполните обычное полное обновление Arch; частичное обновление
пакетов через `pacman -Sy` не применяется.

```bash
sudo pacman -Syu
sudo pacman -S --needed git base-devel webkit2gtk-4.1 openssl \
  appmenu-gtk-module libappindicator librsvg xdotool curl wget file npm
```

Также нужны **Node.js 22.12+** (рекомендуется 24 LTS) и стабильный
**Rust 1.90+** с Cargo. Если они уже установлены через менеджер версий,
используйте существующие установки. Для новой среды без Node/Rust:

```bash
sudo pacman -S --needed nodejs-lts-krypton rustup
rustup default stable
```

`nodejs-lts-krypton` — пакет Node 24. При установленном другом Node
не меняйте его, если `node --version` удовлетворяет требованию. Системный
пакет `rust` — ещё один вариант вместо `rustup`; одновременно ставить
оба не нужно. Проверка:

```bash
node --version
npm --version
cargo --version
rustc --version
pkg-config --modversion webkit2gtk-4.1 gtk+-3.0
```

## Сборка и установка приложения

```bash
git clone https://github.com/webar4i/aegis-protocol.git
cd aegis-protocol
node platforms/linux/install.mjs
```

Если репозиторий уже есть, перейдите в него и обновите через
`git pull --ff-only`, сохранив собственные изменения. Установщик выполняет
`npm ci`, тесты UI и desktop-сборку с закреплённым Cargo.lock. При первом
запуске скачиваются Rust/JavaScript-зависимости и компилируется WebView-оболочка;
нужны интернет, свободное место и время на компиляцию.
Установщик по умолчанию ограничивает Cargo двумя задачами; при необходимости
задайте `CARGO_BUILD_JOBS` явно.

В меню появится **Aegis (Development)**. Запуск из терминала:

```bash
~/.local/bin/aegis-client
```

Приложение использует локально встроенные web-ресурсы и системный WebKitGTK;
Vite-сервер для установленной версии не нужен. Пользовательская установка:

| Файл | Путь по умолчанию |
| --- | --- |
| Бинарный файл и значок | `~/.local/share/aegis-client/` |
| Ссылка для запуска | `~/.local/bin/aegis-client` |
| Ярлык меню | `~/.local/share/applications/dev.aegisprotocol.client.desktop` |

`XDG_DATA_HOME` учитывается, если содержит абсолютный путь. Установщик не
запрашивает sudo, не меняет маршруты, DNS, автозагрузку или системные службы.
Не запускайте сам GUI от root. Это локальная developer-установка,
а не пакет pacman/AUR и не самостоятельно обновляемый релиз.

## Разработка

Из каталога проекта:

```bash
cd apps/ionic
npm ci
npm run desktop:dev
```

Откроется окно с devtools и обновлением интерфейса при изменениях.
Dev-сервер слушает только `127.0.0.1:5173`. Изменения Rust-оболочки
перекомпилируются Tauri. Для одних web-экранов достаточно `npm run dev`.
Изменения в dev-окне не заменяют установленную копию: после работы выполните
`node platforms/linux/install.mjs` из корня и перезапустите приложение.
Если desktop binary уже собран командой `npm run desktop:build`, повторную
установку без компиляции выполняет `node platforms/linux/install.mjs --no-build`.

Для работы над протоколом отдельно запустите Rust CLI по [README](../README.md).
Подключение его сессий к UI и настоящий Linux TUN — следующие этапы.

## Удаление

```bash
node platforms/linux/install.mjs --uninstall
```

Удаляются установленный бинарный файл, значок и ярлык. Исходники,
toolchain и пользовательские настройки WebView сохраняются.

## Если запуск не удался

- `cargo`/`rustc` не найдены: включите установленный Rust в PATH, откройте
  новый терминал; для rustup проверьте `rustup show`.
- `pkg-config`/GTK/WebKit не найдены: проверьте пакеты первого шага.
  Нужен именно `webkit2gtk-4.1`.
- Ошибка `cannot open display`: запускайте из терминала графического сеанса
  обычного пользователя, не через голый SSH и не через sudo.
- На Wayland при проблемах драйвера можно проверить XWayland:
  `GDK_BACKEND=x11 ~/.local/bin/aegis-client` (нужен работающий XWayland).
  Не отключайте WebKit sandbox ради обхода ошибки.
- Если процесс завершился с ошибкой, сохраните её текст из терминала.
  На текущем этапе нет VPN-секретов, но личные пути можно скрыть перед отправкой.

Сборка Linux и тесты установщика не доказывают работу на конкретном Arch,
драйвере или Wayland-композиторе. Фактические проверки фиксируются отдельно
в [CLIENT_VALIDATION.md](CLIENT_VALIDATION.md).

Проверенные первичные источники (2026-10-21):
[Tauri prerequisites](https://v2.tauri.app/start/prerequisites/),
[Arch WebKitGTK](https://archlinux.org/packages/extra/x86_64/webkit2gtk-4.1/),
[Arch libappindicator](https://archlinux.org/packages/extra/x86_64/libappindicator/),
[Arch Node 24](https://archlinux.org/packages/extra/x86_64/nodejs-lts-krypton/),
[XDG paths](https://specifications.freedesktop.org/basedir/latest/),
[Desktop Exec format](https://specifications.freedesktop.org/desktop-entry/latest/exec-variables.html).
