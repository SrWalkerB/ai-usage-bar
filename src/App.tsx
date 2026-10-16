import { listen } from "@tauri-apps/api/event";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { type AppView, type ProviderId, api } from "./api";
import { AddProviderView, ConnectView } from "./components/Connect";
import { ProviderCard } from "./components/ProviderCard";
import { SettingsView } from "./components/SettingsView";
import { BackIcon, DotsIcon, GearIcon, PlusIcon, RefreshIcon, SparkleLogo } from "./components/icons";
import { formatAgo } from "./format";
import { I18nContext, makeT, resolveLocale } from "./i18n";
import "./App.css";

type Screen = { name: "home" } | { name: "add" } | { name: "connect"; provider: ProviderId } | { name: "settings" };

/** Re-render periodically so relative times ("2 min ago") stay fresh. */
function useTick(ms: number) {
  const [, set] = useState(0);
  useEffect(() => {
    const id = setInterval(() => set((n) => n + 1), ms);
    return () => clearInterval(id);
  }, [ms]);
}

export default function App() {
  const [view, setView] = useState<AppView | null>(null);
  const [screen, setScreen] = useState<Screen>({ name: "home" });
  const [syncing, setSyncing] = useState(false);
  const [menuOpen, setMenuOpen] = useState(false);
  const panelRef = useRef<HTMLDivElement>(null);
  useTick(30_000);

  const locale = resolveLocale(view?.settings.language ?? "system");
  const i18n = useMemo(() => ({ t: makeT(locale), locale }), [locale]);
  const { t } = i18n;

  useEffect(() => {
    api.getState().then(setView);
    const unlisten = [
      listen<AppView>("state-changed", (e) => setView(e.payload)),
      listen("window-shown", () => setMenuOpen(false)),
    ];
    return () => unlisten.forEach((p) => p.then((f) => f()));
  }, []);

  // Size the native window to fit the panel. Also re-fit whenever the window is shown,
  // since a hidden webview may skip or misreport layout.
  useLayoutEffect(() => {
    const el = panelRef.current;
    if (!el) return;
    const fit = () => {
      const h = Math.ceil(el.getBoundingClientRect().height);
      if (h > 50) api.fitWindow(h);
    };
    const ro = new ResizeObserver(fit);
    ro.observe(el);
    const shown = listen("window-shown", () => requestAnimationFrame(fit));
    return () => {
      ro.disconnect();
      shown.then((f) => f());
    };
  }, []);

  useEffect(() => {
    document.documentElement.lang = locale;
  }, [locale]);

  const sync = async () => {
    setSyncing(true);
    try {
      setView(await api.refresh());
    } finally {
      setSyncing(false);
    }
  };

  const connected = view?.providers.filter((p) => p.connected) ?? [];
  const goHome = (v?: AppView) => {
    if (v) setView(v);
    setScreen({ name: "home" });
  };

  const title =
    screen.name === "settings"
      ? t("settings")
      : screen.name === "add" || screen.name === "connect"
        ? t("addProvider")
        : "AI Usage Monitor";

  return (
    <I18nContext.Provider value={i18n}>
      <div className="panel" ref={panelRef}>
        <header className="top">
          {screen.name === "home" ? (
            <SparkleLogo />
          ) : (
            <button
              className="icon-btn"
              aria-label={t("back")}
              onClick={() => setScreen(screen.name === "connect" ? { name: "add" } : { name: "home" })}
            >
              <BackIcon />
            </button>
          )}
          <div className="top__titles">
            <h1>{title}</h1>
          </div>
          {screen.name === "home" && connected.length > 0 && (
            <span className="pill">
              <span className="dot" />
              {connected.length === 1 ? t("connectedOne") : t("connected", { n: connected.length })}
            </span>
          )}
        </header>

        <main className="content">
          {screen.name === "home" && (
            <>
              {view && connected.length === 0 && (
                <div className="empty">
                  <strong>{t("emptyTitle")}</strong>
                  <span className="muted">{t("emptyHint")}</span>
                </div>
              )}
              {connected.map((p) => (
                <ProviderCard key={p.id} p={p} onReconnect={() => setScreen({ name: "connect", provider: p.id })} />
              ))}
              {view && connected.length < view.providers.length && (
                <button className="add" onClick={() => setScreen({ name: "add" })}>
                  <PlusIcon size={22} />
                  {t("addProvider")}
                </button>
              )}
            </>
          )}

          {screen.name === "add" && view && (
            <AddProviderView providers={view.providers} onPick={(p) => setScreen({ name: "connect", provider: p.id })} />
          )}

          {screen.name === "connect" && view && (
            <ConnectView
              key={screen.provider}
              provider={view.providers.find((p) => p.id === screen.provider)!}
              onDone={goHome}
            />
          )}

          {screen.name === "settings" && view && <SettingsView settings={view.settings} onChange={setView} />}
        </main>

        <footer className="foot">
          <button className="icon-btn" onClick={sync} disabled={syncing} aria-label="Refresh">
            <span className={syncing ? "spin" : ""}>
              <RefreshIcon size={20} />
            </span>
          </button>
          <span className="muted">
            {syncing
              ? t("syncing")
              : view?.lastSync
                ? t("lastSync", { time: formatAgo(view.lastSync, locale) })
                : t("notSynced")}
          </span>
          <span className="spacer" />
          <button
            className={`icon-btn ${screen.name === "settings" ? "icon-btn--active" : ""}`}
            aria-label={t("settings")}
            onClick={() => setScreen(screen.name === "settings" ? { name: "home" } : { name: "settings" })}
          >
            <GearIcon size={20} />
          </button>
          <span className="sep" />
          <div className="menu-wrap">
            <button className="icon-btn" aria-label="Menu" onClick={() => setMenuOpen((o) => !o)}>
              <DotsIcon size={20} />
            </button>
            {menuOpen && (
              <div className="menu" onMouseLeave={() => setMenuOpen(false)}>
                {connected.map((p) => (
                  <button
                    key={p.id}
                    onClick={async () => {
                      setMenuOpen(false);
                      setView(await api.disconnect(p.id));
                    }}
                  >
                    {t("disconnect", { name: p.name })}
                  </button>
                ))}
                <button className="danger" onClick={() => api.quit()}>
                  {t("quit")}
                </button>
              </div>
            )}
          </div>
        </footer>
      </div>
    </I18nContext.Provider>
  );
}
