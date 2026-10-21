# Aegis Client · Ionic React + Capacitor

Работающий интерфейс клиента на Ionic React 9, React 19, TypeScript и Vite.
Capacitor 8 содержит исходные проекты Android и iOS. Экран соединения, выбор
примера сервера, поиск, настройки и справка работают на мобильных и настольных
размерах экрана.

**VPN-модуль ещё не подключён.** Приложение не создаёт системный туннель,
не перенаправляет трафик, не авторизует пользователей и не показывает выдуманные
соединения или измерения. Серверы в интерфейсе явно обозначены как примеры.
Нажатие кнопки подключения открывает описание текущего состояния.

## Запуск

Нужен Node.js **22.12+** (рекомендуется Node 24).

```bash
cd apps/ionic
npm ci
npm run dev
```

Откройте `http://127.0.0.1:5173`. Сервер разработки доступен только локально.
Для проверки на своём телефоне в доверенной сети можно явно запустить
`npm run dev -- --host 0.0.0.0`. Для проверки сборки:

```bash
npm test
npm run build
npm run preview
npm audit
```

Сборка создаёт `dist/`; эта директория не включается в Git. Внешние шрифты,
аналитика, трекеры и внешние картинки не загружаются.

## Android

Нужны Android Studio, JDK 21 и Android SDK 36. Исходный проект уже включён.

```bash
npm run build
npx cap sync android
npx cap open android
```

Или собрать отладочный APK в терминале с установленным SDK:

```bash
cd android
./gradlew assembleDebug
```

На Windows: `gradlew.bat assembleDebug`. APK будет в
`android/app/build/outputs/apk/debug/app-debug.apk`. Это приложение с
интерфейсом, а не работающий Android VPN: VpnService и JNI-адаптер Rust
ещё предстоит реализовать.

## iOS

Нужны macOS, Xcode 26+, iOS 15+ и настройка подписи разработчика.
Проект использует Swift Package Manager.

```bash
npm run build
npx cap sync ios
npx cap open ios
```

Network Extension и Packet Tunnel Provider ещё не реализованы; соответствующее
разрешение и подпись потребуются до настоящего VPN. Сборка iOS на Linux
не проверяется.

Для повторного создания платформ в чистом проекте без каталогов `android/`
и `ios/`: `npx cap add android`, `npx cap add ios`, затем `npx cap sync`.

## Linux desktop / Arch

Добавлена оболочка **Tauri 2**: она открывает тот же Ionic-интерфейс в отдельном
окне, а release-сборка включает web-ресурсы. Capacitor продолжает обслуживать
Android/iOS. Требуются Rust 1.90+, Node 22.12+ и системные GTK/WebKit-зависимости.
Полная установка с ярлыком: [инструкция Arch Linux](../../docs/ARCH_LINUX.md).

```bash
npm ci
npm run desktop:dev
```

Сборка: `npm run desktop:build`. Результат:
`src-tauri/target/release/aegis-client-desktop`. Установить из корня репозитория:
`node platforms/linux/install.mjs --no-build`. Обычная команда
`node platforms/linux/install.mjs` также ставит npm-зависимости, запускает
UI-тесты и собирает бинарник перед установкой.

Desktop crate имеет отдельный Cargo workspace/lock, поэтому GTK-зависимости
не попадают в portable Rust CLI. WebView получает только локальные ресурсы;
Tauri capabilities пусты, shell/fs-плагины и собственные IPC-команды не
зарегистрированы. В UI сохраняется unsupported fallback: **VPN ещё нет**.
Windows/macOS desktop-сборки и системные VPN-адаптеры ещё не проверены.

## Контракт системного модуля

`src/bridge/aegis.ts` регистрирует Capacitor-плагин `AegisVpn`:

| Метод | Назначение |
| --- | --- |
| `getCapabilities()` | Наличие модуля и поддержка возможностей ОС |
| `getStatus()` | Фактический статус туннеля и реальные счётчики |
| `connect({ profileId, serverId })` | Подключение по ID профиля в защищённом системном хранилище |
| `disconnect()` | Завершение подключения |

Пока нативный плагин отсутствует, используется fallback с
`available: false`, `state: unsupported`, `tunnelActive: false`.
Вызов `connect()` возвращает ошибку `VPN_MODULE_UNAVAILABLE`.
Интерфейс никогда не считает соединение защищённым без одновременно
`state: connected` и `tunnelActive: true` от системного модуля.

Секреты и профили не сохраняются в `localStorage`. Там находятся только ID
выбранного примера сервера и настройка движения. Нативные ключи должны
храниться через Android Keystore / Apple Keychain; формат Rust FFI нужно
зафиксировать перед реализацией адаптеров.

## Зависимости и проверки

Версии зависимостей закреплены в `package.json` и `package-lock.json`.
Для транзитивной build-зависимости `xcode` используется `uuid` 11.1.1 вместо
уязвимой версии 7 (GHSA-w5hq-g745-h8pq). Совместимость вызова `uuid.v4()`
проверяется при генерации и синхронизации проекта.

Тесты Vitest проверяют отказ недоступного VPN, отсутствие ложного состояния
защиты, валидацию нативного статуса/счётчиков и ограничение сохраняемых
настроек. Сборка TypeScript и web bundle проверяются отдельно от нативной
компиляции; успешная web-сборка не означает успешную сборку APK/IPA.

Документация компонентов: https://ionicframework.com/docs/components.
