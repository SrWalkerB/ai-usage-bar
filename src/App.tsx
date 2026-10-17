import { listen } from "@tauri-apps/api/event";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { type AppView, type ProviderId, api } from "./api";
import { AddProviderView, ConnectView } from "./components/Connect";
import { ProviderCard } from "./components/ProviderCard";
import { SettingsView } from "./components/SettingsView";
import { BackIcon, GearIcon, PlusIcon, RefreshIcon } from "./components/icons";
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
  const panelRef = useRef<HTMLDivElement>(null);
  useTick(30_000);

  const locale = resolveLocale(view?.settings.language ?? "system");
  const i18n = useMemo(() => ({ t: makeT(locale), locale }), [locale]);
  const { t } = i18n;

  useEffect(() => {
    api.getState().then(setView);
    const unlisten = listen<AppView>("state-changed", (e) => setView(e.payload));
    return () => {
      unlisten.then((f) => f());
    };
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
  const toggle = (name: "settings" | "add") =>
    setScreen(screen.name === name ? { name: "home" } : name === "add" ? { name: "add" } : { name: "settings" });

  const subScreenTitle =
    screen.name === "settings" ? t("settings") : screen.name === "home" ? null : t("addProvider");

  return (
    <I18nContext.Provider value={i18n}>
      <div className="panel" ref={panelRef}>
        <header className="top">
          {subScreenTitle ? (
            <>
              <button
                className="icon-btn icon-btn--back"
                aria-label={t("back")}
                onClick={() => setScreen(screen.name === "connect" ? { name: "add" } : { name: "home" })}
              >
                <BackIcon size={20} />
              </button>
              <h1 className="top__title">{subScreenTitle}</h1>
            </>
          ) : (
            <div className="top__titles">
              <h1 className="top__title">AI Usage</h1>
              <p>{connected.length === 1 ? t("connectedOne") : t("connected", { n: connected.length })}</p>
            </div>
          )}
          <span className="spacer" />
          <button
            className={`icon-btn ${screen.name === "settings" ? "icon-btn--active" : ""}`}
            aria-label={t("settings")}
            onClick={() => toggle("settings")}
          >
            <GearIcon size={22} />
          </button>
          <button
            className={`icon-btn ${screen.name === "add" || screen.name === "connect" ? "icon-btn--active" : ""}`}
            aria-label={t("addProvider")}
            onClick={() => toggle("add")}
          >
            <PlusIcon size={24} />
          </button>
        </header>

        <main className="content">
          {screen.name === "home" && (
            <>
              {view && connected.length === 0 && (
                <div className="card empty">
                  <strong>{t("emptyTitle")}</strong>
                  <span className="muted">{t("emptyHint")}</span>
                  <button className="btn btn--primary" onClick={() => setScreen({ name: "add" })}>
                    {t("addProvider")}
                  </button>
                </div>
              )}
              {connected.map((p) => (
                <ProviderCard key={p.id} p={p} onReconnect={() => setScreen({ name: "connect", provider: p.id })} />
              ))}
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

          {screen.name === "settings" && view && <SettingsView view={view} onChange={setView} />}
        </main>

        <footer className="foot">
          <button className="foot__sync" onClick={sync} disabled={syncing}>
            <span className={syncing ? "spin" : ""}>
              <RefreshIcon size={22} />
            </span>
            {syncing
              ? t("syncing")
              : view?.lastSync
                ? t("lastSync", { time: formatAgo(view.lastSync, locale) })
                : t("notSynced")}
          </button>
          <span className="spacer" />
          <button className="foot__quit" onClick={() => api.quit()}>
            {t("quit")}
          </button>
        </footer>
      </div>
    </I18nContext.Provider>
  );
}
