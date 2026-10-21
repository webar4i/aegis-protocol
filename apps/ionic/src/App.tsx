import { useEffect, useMemo, useState } from "react";
import {
  IonApp,
  IonBadge,
  IonButton,
  IonButtons,
  IonCard,
  IonCardContent,
  IonContent,
  IonFooter,
  IonHeader,
  IonIcon,
  IonItem,
  IonLabel,
  IonList,
  IonModal,
  IonNote,
  IonSearchbar,
  IonSegment,
  IonSegmentButton,
  IonTitle,
  IonToggle,
  IonToolbar,
} from "@ionic/react";
import {
  arrowDownOutline,
  arrowForwardOutline,
  arrowUpOutline,
  checkmarkOutline,
  chevronForwardOutline,
  closeOutline,
  codeSlashOutline,
  compassOutline,
  globeOutline,
  helpCircleOutline,
  informationCircleOutline,
  keyOutline,
  lockClosedOutline,
  optionsOutline,
  powerOutline,
  refreshOutline,
  serverOutline,
  settingsOutline,
  shieldCheckmarkOutline,
  shieldOutline,
  sparklesOutline,
  terminalOutline,
} from "ionicons/icons";
import {
  createVpnBridge,
  isProtected,
  unavailableStatus,
  validateStatus,
} from "./bridge/aegis";
import type { VpnCapabilities, VpnStatus } from "./bridge/aegis";
import { defaults, loadPreferences, savePreferences } from "./preferences";
import type { Preferences, ServerId } from "./preferences";

type Page = "connection" | "servers" | "settings" | "about";
const pages: { id: Page; label: string; icon: string }[] = [
  { id: "connection", label: "Подключение", icon: shieldOutline },
  { id: "servers", label: "Серверы", icon: globeOutline },
  { id: "settings", label: "Настройки", icon: optionsOutline },
];
const servers: { id: ServerId; name: string; city: string; code: string }[] = [
  {
    id: "auto",
    name: "Автоматически",
    city: "Доступный сервер с лучшим откликом",
    code: "AUTO",
  },
  { id: "nl", name: "Нидерланды", city: "Амстердам", code: "NL" },
  { id: "de", name: "Германия", city: "Франкфурт", code: "DE" },
  { id: "fi", name: "Финляндия", city: "Хельсинки", code: "FI" },
];
function initialPreferences(): Preferences {
  try {
    return loadPreferences(window.localStorage);
  } catch {
    return { ...defaults };
  }
}
function ServerIcon({ id }: { id: ServerId }) {
  return id === "auto" ? (
    <span className="server-flag auto-flag">
      <IonIcon icon={sparklesOutline} />
    </span>
  ) : (
    <span className={`server-flag flag-${id}`} aria-hidden="true" />
  );
}
function Logo({ small = false }: { small?: boolean }) {
  return (
    <span className={`brand ${small ? "brand-small" : ""}`}>
      <span className="brand-mark">
        <IonIcon icon={shieldOutline} />
        <span>A</span>
      </span>
      <span className="brand-type">
        AEGIS<span>PRIVATE BY DESIGN</span>
      </span>
    </span>
  );
}

export default function App() {
  const bridge = useMemo(() => createVpnBridge(), []);
  const [page, setPage] = useState<Page>("connection");
  const [preferences, setPreferences] =
    useState<Preferences>(initialPreferences);
  const [status, setStatus] = useState<VpnStatus>(unavailableStatus);
  const [capabilities, setCapabilities] = useState<VpnCapabilities>({
    available: false,
    platform: "web",
    killSwitch: false,
    splitTunnel: false,
  });
  const [modal, setModal] = useState<"setup" | "privacy" | null>(null);
  const [query, setQuery] = useState("");
  const [serverFilter, setServerFilter] = useState("all");
  const selectedServer = servers.find(
    (server) => server.id === preferences.serverId,
  )!;
  const protectedNow = isProtected(status);

  async function refreshStatus() {
    try {
      const [nextCapabilities, nextStatus] = await Promise.all([
        bridge.getCapabilities(),
        bridge.getStatus(),
      ]);
      setCapabilities(nextCapabilities);
      setStatus(validateStatus(nextStatus));
    } catch {
      setCapabilities({
        available: false,
        platform: "unknown",
        killSwitch: false,
        splitTunnel: false,
      });
      setStatus({
        ...unavailableStatus(),
        state: "error",
        errorCode: "STATUS_UNAVAILABLE",
      });
    }
  }
  useEffect(() => {
    void refreshStatus();
  }, [bridge]);
  useEffect(() => {
    try {
      savePreferences(window.localStorage, preferences);
    } catch {
      /* Storage is optional. */
    }
  }, [preferences]);
  async function connectAction() {
    if (protectedNow) {
      try {
        setStatus(validateStatus(await bridge.disconnect()));
      } catch {
        setStatus({ ...unavailableStatus(), state: "error" });
      }
    } else {
      // The catalog is illustrative and contains no usable profile or endpoint.
      // Never invoke native connect with a manufactured profile or fake success.
      setModal("setup");
    }
  }
  function chooseServer(serverId: ServerId) {
    setPreferences((current) => ({ ...current, serverId }));
    setPage("connection");
  }
  const statusText = protectedNow
    ? "Соединение защищено"
    : status.state === "error"
      ? "Статус недоступен"
      : "Не подключено";
  const filtered = servers.filter(
    (server) =>
      (serverFilter === "all" || server.id !== "auto") &&
      `${server.name} ${server.city}`
        .toLowerCase()
        .includes(query.toLowerCase()),
  );

  return (
    <IonApp className={preferences.reduceMotion ? "reduce-motion" : ""}>
      <div className="app-shell">
        <IonHeader className="ion-no-border topbar">
          <IonToolbar>
            <div slot="start" className="header-brand">
              <Logo />
            </div>
            <div className="header-caption">ВАШЕ СОЕДИНЕНИЕ. ВАШ КОНТРОЛЬ.</div>
            <IonButtons slot="end">
              <span className="build-badge">
                PREVIEW <span>0.1</span>
              </span>
              <IonButton
                aria-label="О приложении"
                onClick={() => setPage("about")}
                className="help-icon"
              >
                <IonIcon slot="icon-only" icon={helpCircleOutline} />
              </IonButton>
            </IonButtons>
          </IonToolbar>
        </IonHeader>
        <IonContent>
          <div className="app-layout">
            <aside className="sidebar">
              <div className="nav-caption">РАБОЧЕЕ ПРОСТРАНСТВО</div>
              <nav aria-label="Главное меню">
                {pages.map((item) => (
                  <IonButton
                    key={item.id}
                    fill="clear"
                    className={`nav-item ${page === item.id ? "active" : ""}`}
                    onClick={() => setPage(item.id)}
                  >
                    <IonIcon icon={item.icon} slot="start" />
                    {item.label}
                    <span className="nav-dot" />
                  </IonButton>
                ))}
              </nav>
              <div className="sidebar-bottom">
                <div className="sidebar-note">
                  <span className="tiny-shield">
                    <IonIcon icon={lockClosedOutline} />
                  </span>
                  <p>
                    Открытый код.
                    <br />
                    <strong>Прозрачный подход.</strong>
                  </p>
                </div>
                <IonButton
                  fill="clear"
                  className={`nav-item about-link ${page === "about" ? "active" : ""}`}
                  onClick={() => setPage("about")}
                >
                  <IonIcon slot="start" icon={informationCircleOutline} />О
                  проекте
                </IonButton>
                <div className="sidebar-version">
                  AEGIS PROTOCOL <span>v0.1.0</span>
                </div>
              </div>
            </aside>

            <main className="main-content">
              <div className="page-heading">
                <div>
                  <div className="eyebrow">AEGIS CLIENT</div>
                  <h1>
                    {page === "connection"
                      ? "Ваше соединение"
                      : page === "servers"
                        ? "Точка подключения"
                        : page === "settings"
                          ? "Настройки"
                          : "Создан для свободы"}
                  </h1>
                  <p>
                    {page === "connection"
                      ? "Чуть больше приватности. Чуть меньше границ."
                      : page === "servers"
                        ? "Выберите, откуда начнётся ваш маршрут."
                        : page === "settings"
                          ? "Приложение, которое работает по вашим правилам."
                          : "Независимый протокол. Открытая разработка."}
                  </p>
                </div>
                <span
                  className={`connection-indicator ${protectedNow ? "is-protected" : ""}`}
                >
                  <span />
                  {protectedNow ? "Защищено" : "VPN выключен"}
                </span>
              </div>

              {page === "connection" && (
                <>
                  <div className="connection-grid">
                    <IonCard className="panel connection-panel">
                      <IonCardContent>
                        <div className="panel-top">
                          <span className="panel-kicker">
                            ЗАЩИЩЁННЫЙ МАРШРУТ
                          </span>
                          <span className="protocol-label">
                            <span />
                            AEGIS / TLS 1.3
                          </span>
                        </div>
                        <div className="connection-center">
                          <div className="orbital orbital-one" />
                          <div className="orbital orbital-two" />
                          <div className="orbital orbital-three" />
                          <div className="power-wrap">
                            <IonButton
                              className="power-button"
                              aria-label={
                                protectedNow
                                  ? "Отключиться от VPN"
                                  : "Подключиться к VPN"
                              }
                              onClick={() => void connectAction()}
                            >
                              <IonIcon icon={powerOutline} />
                            </IonButton>
                          </div>
                          <h2>
                            {protectedNow ? "Отключиться" : "Подключиться"}
                          </h2>
                          <div className="connection-state">
                            <span />
                            {statusText}
                          </div>
                        </div>
                        <IonButton
                          fill="clear"
                          className="selected-server"
                          onClick={() => setPage("servers")}
                          aria-label="Выбрать сервер"
                        >
                          <ServerIcon id={selectedServer.id} />
                          <span className="selected-copy">
                            <small>ВЫБРАННЫЙ СЕРВЕР</small>
                            <strong>{selectedServer.name}</strong>
                          </span>
                          <span className="server-sample">Пример</span>
                          <IonIcon icon={chevronForwardOutline} />
                        </IonButton>
                      </IonCardContent>
                    </IonCard>

                    <div className="session-column">
                      <IonCard className="panel session-panel">
                        <IonCardContent>
                          <div className="card-heading">
                            <span className="panel-kicker">ТЕКУЩИЙ СЕАНС</span>
                            <IonIcon icon={shieldOutline} />
                          </div>
                          <div className="session-time">
                            —<span>Время подключения</span>
                          </div>
                          <div className="traffic-grid">
                            <div>
                              <span>
                                <IonIcon icon={arrowDownOutline} />
                                Получено
                              </span>
                              <strong>
                                {status.receivedBytes === null
                                  ? "—"
                                  : `${status.receivedBytes} Б`}
                              </strong>
                            </div>
                            <div>
                              <span>
                                <IonIcon icon={arrowUpOutline} />
                                Отправлено
                              </span>
                              <strong>
                                {status.sentBytes === null
                                  ? "—"
                                  : `${status.sentBytes} Б`}
                              </strong>
                            </div>
                          </div>
                          <div className="session-detail">
                            <span>Ваш IP-адрес</span>
                            <strong>Не определён</strong>
                          </div>
                          <div className="session-detail">
                            <span>Защита трафика</span>
                            <strong className={protectedNow ? "cyan-text" : ""}>
                              {protectedNow ? "Активна" : "Не активна"}
                            </strong>
                          </div>
                        </IonCardContent>
                      </IonCard>
                      <IonCard className="panel privacy-card">
                        <IonCardContent>
                          <IonIcon icon={lockClosedOutline} />
                          <h3>Приватность по умолчанию</h3>
                          <p>
                            Содержимое вашего трафика не записывается. Всё, что
                            хранит приложение, — ваши настройки.
                          </p>
                          <IonButton
                            fill="clear"
                            onClick={() => setModal("privacy")}
                          >
                            Как это устроено
                            <IonIcon slot="end" icon={arrowForwardOutline} />
                          </IonButton>
                        </IonCardContent>
                      </IonCard>
                    </div>
                  </div>
                  <div className="notice-strip">
                    <span className="notice-icon">
                      <IonIcon icon={codeSlashOutline} />
                    </span>
                    <div>
                      <strong>Начало большого пути</strong>
                      <p>
                        Это предварительная версия. VPN-модуль ещё не подключён
                        — трафик идёт через обычное соединение.
                      </p>
                    </div>
                    <IonButton
                      fill="clear"
                      onClick={() => setPage("about")}
                      aria-label="Подробнее о версии"
                    >
                      <IonIcon slot="icon-only" icon={arrowForwardOutline} />
                    </IonButton>
                  </div>
                  <div className="bottom-caption">
                    <span>
                      <IonIcon icon={shieldCheckmarkOutline} />
                      Проверенное шифрование. Собственный протокол.
                    </span>
                    <span>СДЕЛАНО С УВАЖЕНИЕМ К ВАШЕЙ ПРИВАТНОСТИ</span>
                  </div>
                </>
              )}

              {page === "servers" && (
                <div className="servers-page">
                  <IonCard className="panel server-list-card">
                    <IonCardContent>
                      <div className="list-heading">
                        <h2>Серверы</h2>
                        <IonBadge className="subtle-badge">Примеры</IonBadge>
                      </div>
                      <p className="catalog-note">
                        Каталог для знакомства с интерфейсом. Реальные серверы
                        появятся после добавления профиля.
                      </p>
                      <IonSearchbar
                        value={query}
                        onIonInput={(event) =>
                          setQuery(event.detail.value ?? "")
                        }
                        placeholder="Найти страну или город"
                        className="server-search"
                      />
                      <IonSegment
                        value={serverFilter}
                        onIonChange={(event) =>
                          setServerFilter(String(event.detail.value))
                        }
                      >
                        <IonSegmentButton value="all">
                          <IonLabel>Все</IonLabel>
                        </IonSegmentButton>
                        <IonSegmentButton value="countries">
                          <IonLabel>Страны</IonLabel>
                        </IonSegmentButton>
                      </IonSegment>
                      <IonList className="server-list" lines="none">
                        {filtered.map((server) => (
                          <IonItem
                            key={server.id}
                            button
                            detail={false}
                            onClick={() => chooseServer(server.id)}
                            className={
                              preferences.serverId === server.id
                                ? "server-row selected"
                                : "server-row"
                            }
                          >
                            <ServerIcon id={server.id} />
                            <IonLabel>
                              <h3>{server.name}</h3>
                              <p>{server.city}</p>
                            </IonLabel>
                            <span className="row-example">Пример</span>
                            {preferences.serverId === server.id ? (
                              <IonIcon
                                className="selected-check"
                                icon={checkmarkOutline}
                                slot="end"
                              />
                            ) : (
                              <IonIcon
                                icon={chevronForwardOutline}
                                slot="end"
                              />
                            )}
                          </IonItem>
                        ))}
                      </IonList>
                      {filtered.length === 0 && (
                        <div className="empty-state">
                          <IonIcon icon={compassOutline} />
                          <h3>Такой страны пока нет</h3>
                          <p>Попробуйте другой запрос.</p>
                        </div>
                      )}
                    </IonCardContent>
                  </IonCard>
                  <div className="side-explainer">
                    <div className="explainer-icon">
                      <IonIcon icon={globeOutline} />
                    </div>
                    <h2>
                      Ваш маршрут —<br />
                      ваш выбор.
                    </h2>
                    <p>
                      Автоматический выбор будет искать доступный сервер по
                      результатам реальных измерений.
                    </p>
                    <span>В этой версии доступен только выбор примера.</span>
                  </div>
                </div>
              )}

              {page === "settings" && (
                <div className="settings-page">
                  <IonCard className="panel settings-card">
                    <IonCardContent>
                      <div className="list-heading">
                        <h2>Приложение</h2>
                        <IonIcon icon={settingsOutline} />
                      </div>
                      <IonList lines="full">
                        <IonItem>
                          <IonLabel>
                            <h3>Уменьшить движение</h3>
                            <p>Меньше переходов и визуальных эффектов</p>
                          </IonLabel>
                          <IonToggle
                            aria-label="Уменьшить движение"
                            checked={preferences.reduceMotion}
                            onIonChange={(event) =>
                              setPreferences((current) => ({
                                ...current,
                                reduceMotion: event.detail.checked,
                              }))
                            }
                          />
                        </IonItem>
                        <IonItem>
                          <IonLabel>
                            <h3>Тема оформления</h3>
                            <p>Тёмная · Aegis Midnight</p>
                          </IonLabel>
                          <span className="theme-swatch" />
                        </IonItem>
                        <IonItem>
                          <IonLabel>
                            <h3>Язык</h3>
                            <p>Русский</p>
                          </IonLabel>
                          <span className="item-value">RU</span>
                        </IonItem>
                      </IonList>
                    </IonCardContent>
                  </IonCard>
                  <IonCard className="panel settings-card">
                    <IonCardContent>
                      <div className="list-heading">
                        <h2>Защита соединения</h2>
                        <IonBadge className="subtle-badge">Скоро</IonBadge>
                      </div>
                      <IonList lines="full">
                        <IonItem>
                          <IonLabel>
                            <h3>Kill Switch</h3>
                            <p>Будет доступен после подключения VPN-модуля</p>
                          </IonLabel>
                          <IonToggle
                            aria-label="Kill Switch недоступен"
                            checked={false}
                            disabled
                          />
                        </IonItem>
                        <IonItem>
                          <IonLabel>
                            <h3>Раздельное туннелирование</h3>
                            <p>Зависит от возможностей вашей ОС</p>
                          </IonLabel>
                          <IonToggle
                            aria-label="Раздельное туннелирование недоступно"
                            checked={false}
                            disabled
                          />
                        </IonItem>
                        <IonItem>
                          <IonLabel>
                            <h3>DNS и IPv6</h3>
                            <p>Защита ещё не реализована</p>
                          </IonLabel>
                          <IonIcon icon={lockClosedOutline} />
                        </IonItem>
                      </IonList>
                    </IonCardContent>
                  </IonCard>
                  <IonCard className="panel settings-card">
                    <IonCardContent>
                      <div className="list-heading">
                        <h2>Состояние приложения</h2>
                        <IonButton
                          fill="clear"
                          aria-label="Обновить состояние"
                          onClick={() => void refreshStatus()}
                        >
                          <IonIcon slot="icon-only" icon={refreshOutline} />
                        </IonButton>
                      </div>
                      <div className="settings-status">
                        <span className="status-dot" />
                        <div>
                          <strong>
                            {capabilities.available
                              ? "VPN-модуль доступен"
                              : "VPN-модуль ещё не подключён"}
                          </strong>
                          <p>
                            {capabilities.available
                              ? "Для подключения потребуется действующий профиль."
                              : "Интерфейс готов. Системный туннель пока недоступен."}
                          </p>
                        </div>
                      </div>
                    </IonCardContent>
                  </IonCard>
                </div>
              )}

              {page === "about" && (
                <div className="about-page">
                  <IonCard className="panel about-card">
                    <IonCardContent>
                      <Logo small />
                      <div className="eyebrow">
                        НЕЗАВИСИМЫЙ ПРОЕКТ / VERSION 0.1
                      </div>
                      <h2>
                        Приватность должна
                        <br />
                        быть понятной.
                      </h2>
                      <p className="about-lead">
                        Aegis объединяет собственный открытый протокол и простой
                        клиент. Мы строим защищённое соединение шаг за шагом и
                        честно показываем, что уже работает.
                      </p>
                      <div className="about-facts">
                        <div>
                          <IonIcon icon={terminalOutline} />
                          <strong>Прототип ядра</strong>
                          <p>
                            Rust, TLS 1.3, взаимная аутентификация и проверяемый
                            двусторонний обмен.
                          </p>
                        </div>
                        <div>
                          <IonIcon icon={serverOutline} />
                          <strong>Следующий шаг</strong>
                          <p>
                            IP-туннель и системная интеграция. Этот интерфейс
                            пока не перенаправляет трафик.
                          </p>
                        </div>
                        <div>
                          <IonIcon icon={keyOutline} />
                          <strong>Без громких обещаний</strong>
                          <p>
                            Ни один протокол не может гарантировать
                            невозможность блокировки.
                          </p>
                        </div>
                      </div>
                      <IonButton
                        className="secondary-action"
                        fill="outline"
                        onClick={() => setModal("privacy")}
                      >
                        Данные и приватность
                        <IonIcon slot="end" icon={arrowForwardOutline} />
                      </IonButton>
                    </IonCardContent>
                  </IonCard>
                </div>
              )}
            </main>
          </div>
        </IonContent>
        <IonFooter className="ion-no-border mobile-nav">
          <IonToolbar>
            <nav aria-label="Главное меню">
              {pages.map((item) => (
                <IonButton
                  fill="clear"
                  key={item.id}
                  className={page === item.id ? "active" : ""}
                  onClick={() => setPage(item.id)}
                >
                  <span className="mobile-tab-content">
                    <IonIcon icon={item.icon} />
                    <span>{item.label}</span>
                  </span>
                </IonButton>
              ))}
            </nav>
          </IonToolbar>
        </IonFooter>

        <IonModal
          isOpen={modal !== null}
          onDidDismiss={() => setModal(null)}
          className="aegis-modal"
        >
          <IonHeader className="ion-no-border">
            <IonToolbar>
              <IonTitle>
                {modal === "privacy"
                  ? "Данные и приватность"
                  : "Первое подключение"}
              </IonTitle>
              <IonButtons slot="end">
                <IonButton aria-label="Закрыть" onClick={() => setModal(null)}>
                  <IonIcon slot="icon-only" icon={closeOutline} />
                </IonButton>
              </IonButtons>
            </IonToolbar>
          </IonHeader>
          <IonContent className="modal-content">
            <div className="modal-inner">
              <div className="modal-symbol">
                <IonIcon
                  icon={modal === "privacy" ? lockClosedOutline : shieldOutline}
                />
              </div>
              {modal === "privacy" ? (
                <>
                  <h2>Только необходимое.</h2>
                  <p>
                    Этот интерфейс хранит на устройстве только выбор примера
                    сервера и настройку движения.
                  </p>
                  <IonList lines="none">
                    <IonItem>
                      <IonIcon slot="start" icon={checkmarkOutline} />
                      <IonLabel className="ion-text-wrap">
                        Содержимое трафика не записывается
                      </IonLabel>
                    </IonItem>
                    <IonItem>
                      <IonIcon slot="start" icon={checkmarkOutline} />
                      <IonLabel className="ion-text-wrap">
                        Пароли и ключи не хранятся в браузере
                      </IonLabel>
                    </IonItem>
                    <IonItem>
                      <IonIcon slot="start" icon={checkmarkOutline} />
                      <IonLabel className="ion-text-wrap">
                        Нет аналитики и сторонних трекеров
                      </IonLabel>
                    </IonItem>
                  </IonList>
                  <IonNote>
                    VPN пока не активен. Сетевые запросы вашего устройства идут
                    через обычное соединение.
                  </IonNote>
                </>
              ) : (
                <>
                  <h2>Почти на старте.</h2>
                  <p>
                    VPN-модуль ещё не подключён. Сейчас можно познакомиться с
                    приложением и выбрать пример сервера.
                  </p>
                  <div className="setup-steps">
                    <div>
                      <span>01</span>
                      <p>
                        <strong>Интерфейс готов</strong>Настройки и выбор
                        сервера уже работают.
                      </p>
                      <IonIcon icon={checkmarkOutline} />
                    </div>
                    <div>
                      <span>02</span>
                      <p>
                        <strong>Системный туннель</strong>Следующий этап
                        разработки Aegis.
                      </p>
                    </div>
                    <div>
                      <span>03</span>
                      <p>
                        <strong>Ваш профиль</strong>Появится вместе с реальными
                        серверами.
                      </p>
                    </div>
                  </div>
                  <IonNote>Соединение сейчас не защищено Aegis.</IonNote>
                </>
              )}
              <IonButton
                expand="block"
                className="modal-action"
                onClick={() => setModal(null)}
              >
                Понятно
              </IonButton>
            </div>
          </IonContent>
        </IonModal>
      </div>
    </IonApp>
  );
}
