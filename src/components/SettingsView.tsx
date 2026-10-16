import { type AppView, type Settings, api } from "../api";
import { LANGUAGES, useI18n } from "../i18n";

const INTERVALS = [1, 5, 15, 30];

export function SettingsView({ settings, onChange }: { settings: Settings; onChange: (v: AppView) => void }) {
  const { t } = useI18n();
  const update = async (patch: Partial<Settings>) => onChange(await api.setSettings({ ...settings, ...patch }));

  return (
    <div className="view settings">
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
  );
}
