import type { T } from "./i18n";

const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;

/** Compact duration: "35 min", "7h", "4 dias". */
export function formatDuration(ms: number, t: T): string {
  const diff = Math.max(0, ms);
  if (diff < HOUR) return t("durMin", { n: Math.max(1, Math.round(diff / MIN)) });
  if (diff < 48 * HOUR) return t("durHour", { n: Math.round(diff / HOUR) });
  const days = Math.round(diff / DAY);
  return days === 1 ? t("durDayOne") : t("durDay", { n: days });
}

/** "2 min ago" / "há 2 min" */
export function formatAgo(ts: number, locale: string, now = Date.now()): string {
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto", style: "short" });
  const diff = Math.max(0, now - ts);
  if (diff < MIN) return rtf.format(0, "second");
  if (diff < HOUR) return rtf.format(-Math.round(diff / MIN), "minute");
  if (diff < DAY) return rtf.format(-Math.round(diff / HOUR), "hour");
  return rtf.format(-Math.round(diff / DAY), "day");
}

/** Exact reset moment: "17:20" when within a day, otherwise "qui., 22/10, 14:34". */
export function formatExact(ts: number, locale: string, now = Date.now()): string {
  const opts: Intl.DateTimeFormatOptions =
    ts - now < DAY
      ? { hour: "2-digit", minute: "2-digit" }
      : { weekday: "short", day: "2-digit", month: "2-digit", hour: "2-digit", minute: "2-digit" };
  return new Intl.DateTimeFormat(locale, opts).format(ts);
}

// API plan identifiers → names shown on the providers' pricing pages.
const PLAN_NAMES: Record<string, Record<string, string>> = {
  codex: {
    free: "Free",
    go: "Go",
    plus: "Plus",
    prolite: "Pro 5x", // the $100 "Pro" tier
    pro: "Pro 20x", // the $200 "Pro" tier
    team: "Business",
    business: "Business",
    enterprise: "Enterprise",
    edu: "Edu",
  },
  claude: { free: "Free", pro: "Pro", max: "Max", team: "Team", enterprise: "Enterprise" },
};

export function formatPlan(provider: string, plan: string | null | undefined): string | null {
  if (!plan) return null;
  return (
    PLAN_NAMES[provider]?.[plan.toLowerCase()] ??
    plan.replace(/[_-]/g, " ").replace(/\b\w/g, (c) => c.toUpperCase())
  );
}
