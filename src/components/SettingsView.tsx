import { type AppView, type Settings, api } from "../api";
import { formatPlan } from "../format";
import { LANGUAGES, useI18n } from "../i18n";
import { ProviderIcon } from "./icons";

const INTERVALS = [1, 5, 15, 30];

export function SettingsView({ view, onChange }: { view: AppView; onChange: (v: AppView) => void }) {
  const { t } = useI18n();
  const settings = view.settings;
  const update = async (patch: Partial<Settings>) => onChange(await api.setSettings({ ...settings, ...patch }));
  const connected = view.providers.filter((p) => p.connected);

  return (
    <div className="view settings">
      <div className="card settings__group">
        <label className="field">
          <span>{t("language")}</span>
          <select className="input" value={settings.language} onChange={(e) => update({ language: e.target.value })}>
            <option value="system">{t("languageSystem")}</option>
            {LANGUAGES.map((l) => (
              <option key={l.code} value={l.code}>
                {l.label}
              </option>
            ))}
          </select>
        </label>

        <div className="field">
          <span>{t("refreshEvery")}</span>
          <div className="segmented">
            {INTERVALS.map((n) => (
              <button key={n} className={settings.refreshMinutes === n ? "active" : ""} onClick={() => update({ refreshMinutes: n })}>
                {t("minutes", { n })}
              </button>
            ))}
          </div>
        </div>

        <label className="field field--row">
          <span>{t("autostart")}</span>
          <input
            type="checkbox"
            className="switch"
            checked={settings.autostart}
            onChange={(e) => update({ autostart: e.target.checked })}
          />
        </label>
      </div>

      {connected.length > 0 && (
        <div className="card settings__group">
          <span className="settings__label">{t("accounts")}</span>
          {connected.map((p) => {
            const plan = formatPlan(p.id, p.usage?.plan);
            return (
              <div key={p.id} className="account">
                <ProviderIcon id={p.id} />
                <span className="account__text">
                  <strong>{p.name}</strong>
                  <span className="muted small">
                    {[plan, p.source === "cli" ? t("viaCli") : t("viaLogin")].filter(Boolean).join(" · ")}
                  </span>
                </span>
                <button className="btn btn--ghost" onClick={async () => onChange(await api.disconnect(p.id))}>
                  {t("disconnectShort")}
                </button>
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}
