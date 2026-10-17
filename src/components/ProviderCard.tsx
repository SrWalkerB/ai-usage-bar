import type { ProviderView, UsageWindow } from "../api";
import { formatDuration, formatExact } from "../format";
import { type TKey, useI18n } from "../i18n";
import { ProviderIcon } from "./icons";

// Bar colors per provider, in window order (shortest window first).
const ACCENTS: Record<string, string[]> = {
  codex: ["blue", "purple", "teal"],
  claude: ["green", "yellow", "teal"],
};

function WindowRow({ w, accent }: { w: UsageWindow; accent: string }) {
  const { t, locale } = useI18n();
  const pct = Math.min(100, Math.max(0, w.usedPercent));
  const tone = pct >= 90 ? "danger" : accent;
  return (
    <div className="window">
      <span className="window__name">
        {t(`window.${w.kind}` as TKey)}
        {w.label && ` · ${w.label}`}
      </span>
      <div className="bar-row">
        <div className={`bar bar--${tone}`} role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
          <div className="bar__fill" style={{ width: `${pct}%` }} />
        </div>
        <span className="bar-pct">{Math.round(pct)}%</span>
      </div>
      {w.resetsAt && (
        <span className="window__reset" title={new Date(w.resetsAt).toLocaleString(locale)}>
          {t("resetsIn", { time: formatDuration(w.resetsAt - Date.now(), t) })}
          <span className="window__exact"> · {formatExact(w.resetsAt, locale)}</span>
        </span>
      )}
    </div>
  );
}

export function ProviderCard({ p, onReconnect }: { p: ProviderView; onReconnect: () => void }) {
  const { t } = useI18n();
  const windows = p.usage?.windows ?? [];
  const accents = ACCENTS[p.id];
  const status = p.error?.kind === "auth" ? "expired" : p.error ? "error" : "active";

  return (
    <section className="card">
      <header className="card__head">
        <ProviderIcon id={p.id} />
        <h2>{p.name}</h2>
        <span className="spacer" />
        <span className={`status status--${status}`} title={p.error?.message}>
          <span className="status__dot" />
          {t(`status.${status}` as TKey)}
        </span>
      </header>

      {windows.map((w, i) => (
        <WindowRow key={`${w.kind}-${w.label ?? ""}`} w={w} accent={accents[Math.min(i, accents.length - 1)]} />
      ))}

      {p.usage && windows.length === 0 && <p className="muted small">{t("noLimits")}</p>}
      {!p.usage && !p.error && <div className="skeleton" />}

      {p.error && (
        <div className={`notice notice--${p.error.kind}`}>
          <span>{p.error.kind === "auth" ? t("authError") : t("fetchError")}</span>
          {p.error.kind === "auth" && (
            <button className="link" onClick={onReconnect}>
              {t("reconnect")}
            </button>
          )}
        </div>
      )}
    </section>
  );
}
