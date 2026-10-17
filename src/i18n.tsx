import { createContext, useContext } from "react";

const en = {
  connected: "{n} connected",
  connectedOne: "1 connected",
  addProvider: "Add provider",
  lastSync: "Updated {time}",
  notSynced: "Not synced yet",
  syncing: "Syncing…",
  resetsIn: "Resets in {time}",
  durMin: "{n} min",
  durHour: "{n}h",
  durDay: "{n} days",
  durDayOne: "1 day",
  "status.active": "Active",
  "status.expired": "Expired",
  "status.error": "Error",
  accounts: "Accounts",
  viaLogin: "signed in",
  disconnectShort: "Disconnect",
  "window.session": "Session (5h)",
  "window.daily": "Daily",
  "window.weekly": "Weekly",
  "window.monthly": "Monthly",
  "window.other": "Limit",
  noLimits: "No limits reported for this plan",
  emptyTitle: "No providers connected",
  emptyHint: "Connect Codex or Claude to start tracking your usage.",
  allConnected: "All available providers are connected.",
  chooseProvider: "Choose a provider",
  "desc.codex": "ChatGPT plan · device code",
  "desc.claude": "Pro / Max plan · sign in",
  settings: "Settings",
  language: "Language",
  languageSystem: "System default",
  refreshEvery: "Refresh every",
  minutes: "{n} min",
  autostart: "Start with Windows",
  back: "Back",
  reconnect: "Reconnect",
  quit: "Quit",
  viaCli: "via CLI",
  authError: "Session expired or revoked.",
  fetchError: "Couldn't update — showing last data.",
  or: "or",
  importCodex: "Use Codex CLI login",
  importClaude: "Use Claude Code login",
  importHint: "Reads the login already saved on this computer (read-only).",
  codexIntro: "Sign in with your ChatGPT account using a one-time code.",
  getCode: "Get code",
  yourCode: "Your code",
  openPage: "Copy code & open page",
  copied: "Copied!",
  waiting: "Waiting for approval in the browser…",
  codexHint: "Paste the code on the page that opens and approve access.",
  claudeIntro: "Open the Claude login, approve access and paste the code shown at the end.",
  openLogin: "Open Claude login",
  pastePlaceholder: "Paste the code here",
  connect: "Connect",
  connecting: "Connecting…",
  cancel: "Cancel",
  error: "Error: {msg}",
  cliNotFound: "No CLI login found on this computer.",
};

type Dict = typeof en;

const ptBR: Dict = {
  connected: "{n} conectados",
  connectedOne: "1 conectado",
  addProvider: "Adicionar provedor",
  lastSync: "Atualizado {time}",
  notSynced: "Ainda não sincronizado",
  syncing: "Sincronizando…",
  resetsIn: "Reseta em {time}",
  durMin: "{n} min",
  durHour: "{n}h",
  durDay: "{n} dias",
  durDayOne: "1 dia",
  "status.active": "Ativo",
  "status.expired": "Expirado",
  "status.error": "Erro",
  accounts: "Contas",
  viaLogin: "login próprio",
  disconnectShort: "Desconectar",
  "window.session": "Sessão (5h)",
  "window.daily": "Diário",
  "window.weekly": "Semanal",
  "window.monthly": "Mensal",
  "window.other": "Limite",
  noLimits: "Nenhum limite informado para este plano",
  emptyTitle: "Nenhum provedor conectado",
  emptyHint: "Conecte o Codex ou o Claude para acompanhar seu uso.",
  allConnected: "Todos os provedores disponíveis já estão conectados.",
  chooseProvider: "Escolha um provedor",
  "desc.codex": "Plano ChatGPT · código de dispositivo",
  "desc.claude": "Plano Pro / Max · login",
  settings: "Configurações",
  language: "Idioma",
  languageSystem: "Padrão do sistema",
  refreshEvery: "Atualizar a cada",
  minutes: "{n} min",
  autostart: "Iniciar com o Windows",
  back: "Voltar",
  reconnect: "Reconectar",
  quit: "Sair",
  viaCli: "via CLI",
  authError: "Sessão expirada ou revogada.",
  fetchError: "Não foi possível atualizar — mostrando últimos dados.",
  or: "ou",
  importCodex: "Usar login do Codex CLI",
  importClaude: "Usar login do Claude Code",
  importHint: "Lê o login já salvo neste computador (somente leitura).",
  codexIntro: "Entre com sua conta do ChatGPT usando um código temporário.",
  getCode: "Gerar código",
  yourCode: "Seu código",
  openPage: "Copiar código e abrir página",
  copied: "Copiado!",
  waiting: "Aguardando aprovação no navegador…",
  codexHint: "Cole o código na página que abrir e aprove o acesso.",
  claudeIntro: "Abra o login do Claude, aprove o acesso e cole o código exibido no final.",
  openLogin: "Abrir login do Claude",
  pastePlaceholder: "Cole o código aqui",
  connect: "Conectar",
  connecting: "Conectando…",
  cancel: "Cancelar",
  error: "Erro: {msg}",
  cliNotFound: "Nenhum login de CLI encontrado neste computador.",
};

export const LANGUAGES = [
  { code: "en", label: "English" },
  { code: "pt-BR", label: "Português (Brasil)" },
];

const dicts: Record<string, Dict> = { en, "pt-BR": ptBR };

export type TKey = keyof Dict;

export function resolveLocale(setting: string): string {
  const lang = setting === "system" ? navigator.language : setting;
  return lang.toLowerCase().startsWith("pt") ? "pt-BR" : "en";
}

export function makeT(locale: string) {
  const dict = dicts[locale] ?? en;
  return (key: TKey, vars: Record<string, string | number> = {}) =>
    dict[key].replace(/\{(\w+)\}/g, (_, k) => String(vars[k] ?? ""));
}

export type T = ReturnType<typeof makeT>;

export const I18nContext = createContext<{ t: T; locale: string }>({ t: makeT("en"), locale: "en" });
export const useI18n = () => useContext(I18nContext);
