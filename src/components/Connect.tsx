import { openUrl } from "@tauri-apps/plugin-opener";
import { useEffect, useRef, useState } from "react";
import { type AppView, type DeviceStart, type ProviderView, api } from "../api";
import { useI18n } from "../i18n";
import { ProviderIcon } from "./icons";

type Props = { provider: ProviderView; onDone: (view: AppView) => void };

const errText = (e: unknown) => (typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e));

function CliImport({ provider, onDone, setError }: Props & { setError: (s: string | null) => void }) {
  const { t } = useI18n();
  const [busy, setBusy] = useState(false);
  if (!provider.cliAvailable) return null;
  const run = async () => {
    setBusy(true);
    setError(null);
    try {
      onDone(await api.importCli(provider.id));
    } catch (e) {
      const msg = errText(e);
      setError(msg === "cli-not-found" ? t("cliNotFound") : t("error", { msg }));
    } finally {
      setBusy(false);
    }
  };
  return (
    <>
      <button className="btn btn--secondary" disabled={busy} onClick={run}>
        {provider.id === "codex" ? t("importCodex") : t("importClaude")}
      </button>
      <p className="muted small center">{t("importHint")}</p>
      <div className="divider">
        <span>{t("or")}</span>
      </div>
    </>
  );
}

function CodexConnect({ provider, onDone }: Props) {
  const { t } = useI18n();
  const [device, setDevice] = useState<DeviceStart | null>(null);
  const [busy, setBusy] = useState(false);
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const active = useRef(true);

  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
      api.codexCancelDevice();
    };
  }, []);

  const start = async () => {
    setBusy(true);
    setError(null);
    try {
      const d = await api.codexStartDevice();
      setDevice(d);
      const view = await api.codexWaitDevice(d);
      if (active.current) onDone(view);
    } catch (e) {
      const msg = errText(e);
      if (active.current && msg !== "cancelled") {
        setError(t("error", { msg }));
        setDevice(null);
      }
    } finally {
      if (active.current) setBusy(false);
    }
  };

  const openPage = async () => {
    if (!device) return;
    try {
      await navigator.clipboard.writeText(device.userCode);
      setCopied(true);
    } catch {
      /* clipboard may be unavailable; the code is visible anyway */
    }
    await openUrl(device.verificationUrl);
  };

  return (
    <div className="connect">
      <CliImport provider={provider} onDone={onDone} setError={setError} />
      <p className="muted">{t("codexIntro")}</p>
      {!device ? (
        <button className="btn btn--primary" disabled={busy} onClick={start}>
          {busy ? t("connecting") : t("getCode")}
        </button>
      ) : (
        <>
          <div className="code-box">
            <span className="muted small">{t("yourCode")}</span>
            <code>{device.userCode}</code>
          </div>
          <button className="btn btn--primary" onClick={openPage}>
            {copied ? t("copied") : t("openPage")}
          </button>
          <p className="muted small center">{t("codexHint")}</p>
          <p className="waiting">
            <span className="spinner" /> {t("waiting")}
          </p>
        </>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}

function ClaudeConnect({ provider, onDone }: Props) {
  const { t } = useI18n();
  const [started, setStarted] = useState(false);
  const [code, setCode] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const open = async () => {
    setError(null);
    const url = await api.claudeStartAuth();
    setStarted(true);
    await openUrl(url);
  };

  const finish = async () => {
    setBusy(true);
    setError(null);
    try {
      onDone(await api.claudeFinishAuth(code));
    } catch (e) {
      setError(t("error", { msg: errText(e) }));
    } finally {
      setBusy(false);
    }
  };

  return (
    <div className="connect">
      <CliImport provider={provider} onDone={onDone} setError={setError} />
      <p className="muted">{t("claudeIntro")}</p>
      <button className={`btn ${started ? "btn--secondary" : "btn--primary"}`} onClick={open}>
        {t("openLogin")}
      </button>
      {started && (
        <form
          className="paste-row"
          onSubmit={(e) => {
            e.preventDefault();
            if (code.trim()) finish();
          }}
        >
          <input
            className="input"
            autoFocus
            value={code}
            onChange={(e) => setCode(e.target.value)}
            placeholder={t("pastePlaceholder")}
            spellCheck={false}
          />
          <button className="btn btn--primary" disabled={busy || !code.trim()}>
            {busy ? t("connecting") : t("connect")}
          </button>
        </form>
      )}
      {error && <p className="error">{error}</p>}
    </div>
  );
}

export function ConnectView(props: Props) {
  return (
    <div className="view">
      <div className="connect-head">
        <ProviderIcon id={props.provider.id} />
        <h2>{props.provider.name}</h2>
      </div>
      {props.provider.id === "codex" ? <CodexConnect {...props} /> : <ClaudeConnect {...props} />}
    </div>
  );
}

export function AddProviderView({ providers, onPick }: { providers: ProviderView[]; onPick: (p: ProviderView) => void }) {
  const { t } = useI18n();
  const available = providers.filter((p) => !p.connected);
  return (
    <div className="view">
      <p className="muted">{available.length ? t("chooseProvider") : t("allConnected")}</p>
      {available.map((p) => (
        <button key={p.id} className="pick" onClick={() => onPick(p)}>
          <ProviderIcon id={p.id} />
          <span className="pick__text">
            <strong>{p.name}</strong>
            <span className="muted small">{t(`desc.${p.id}`)}</span>
          </span>
        </button>
      ))}
    </div>
  );
}
