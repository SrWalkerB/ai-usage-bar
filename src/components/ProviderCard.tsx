import type { ProviderView, UsageWindow } from "../api";
import { formatExact, formatPlan, formatUntil } from "../format";
import { type TKey, useI18n } from "../i18n";
import { AlertIcon, CalendarIcon, ProviderIcon } from "./icons";

const ACCENTS: Record<string, string[]> = {
  codex: ["green", "blue", "teal"],
  claude: ["purple", "blue", "teal"],
};

function ResetLabel({ w }: { w: UsageWindow }) {
  const { t, locale } = useI18n();
  if (!w.resetsAt) return null;
  return (
    <span className="reset" title={new Date(w.resetsAt).toLocaleString(locale)}>
      <CalendarIcon />
      {t("resets", { time: formatUntil(w.resetsAt, locale) })}
      <span className="reset__exact">· {formatExact(w.resetsAt, locale)}</span>
    </span>
  );
}

function Bar({ w, accent }: { w: UsageWindow; accent: string }) {
  const pct = Math.min(100, Math.max(0, w.usedPercent));
  const tone = pct >= 90 ? "danger" : pct >= 75 ? "warn" : accent;
  return (
    <div className="bar-row">
      <div className={`bar bar--${tone}`} role="progressbar" aria-valuenow={Math.round(pct)} aria-valuemin={0} aria-valuemax={100}>
        <div className="bar__fill" style={{ width: `${pct}%` }} />
      </div>
      <span className="bar-pct">{Math.round(pct)}%</span>
    </div>
  );
}

export function ProviderCard({ p, onReconnect }: { p: ProviderView; onReconnect: () => void }) {
  const { t } = useI18n();
  const windows = p.usage?.windows ?? [];
  const accents = ACCENTS[p.id];
  const windowName = (w: UsageWindow) => t(`window.${w.kind}` as TKey) + (w.label ? ` · ${w.label}` : "");
  const plan = formatPlan(p.id, p.usage?.plan);

  return (
    <section className="card">
      <ProviderIcon id={p.id} />
      <div className="card__body">
        <header className="card__head">
          <h2>{p.name}</h2>
          {plan && <span className="chip">{plan}</span>}
          {p.source === "cli" && <span className="chip chip--muted">{t("viaCli")}</span>}
        </header>

        {windows.map((w, i) => (
          <div className="window" key={`${w.kind}-${w.label ?? ""}`}>
            <div className="window__head">
              <span>{windowName(w)}</span>
              <ResetLabel w={w} />
            </div>
            <Bar w={w} accent={accents[Math.min(i, accents.length - 1)]} />
          </div>
        ))}

        {p.usage && windows.length === 0 && <p className="muted small">{t("noLimits")}</p>}
        {!p.usage && !p.error && <div className="skeleton" />}

        {p.error && (
          <div className={`notice notice--${p.error.kind}`} title={p.error.message}>
            <AlertIcon />
            <span>{p.error.kind === "auth" ? t("authError") : t("fetchError")}</span>
            {p.error.kind === "auth" && (
              <button className="link" onClick={onReconnect}>
                {t("reconnect")}
              </button>
            )}
          </div>
        )}
      </div>
    </section>
  );
}
