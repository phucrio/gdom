import { useEffect, useState, type FormEvent } from "react";

import type { BackendPort } from "../ipc/port.ts";
import type { OAuthConfigDto } from "../ipc/types.ts";
import {
  FULL_DRIVE_SCOPE_JUSTIFICATION,
  LIMITED_USE_SENTENCE,
  SYSTEM_BROWSER_OAUTH_EXPLANATION,
} from "../legal/copy.ts";
import { LegalDocument, legalDocumentTitle, type LegalDocumentId } from "../legal/LegalDialogs.tsx";
import { Dialog } from "../ui/Dialog.tsx";
import { GoogleMark } from "./GoogleMark.tsx";
import {
  CONNECT_ADVANCED_HELP,
  CONNECT_ADVANCED_SUMMARY,
  CONNECT_BACK,
  CONNECT_BROWSER_ANNOUNCEMENT,
  CONNECT_CANCEL,
  CONNECT_CLIENT_ID_LABEL,
  CONNECT_CLIENT_ID_REQUIRED,
  CONNECT_CONFIG_LOAD_FAILED,
  CONNECT_CONFIG_LOADING,
  CONNECT_CUSTOM_SAVED,
  CONNECT_FAILED,
  CONNECT_READY_CUSTOM,
  CONNECT_READY_DEFAULT,
  CONNECT_RESET_ANNOUNCEMENT,
  CONNECT_RESET_DEFAULT,
  CONNECT_SAVE_CUSTOM,
  CONNECT_SIGN_IN,
  CONNECT_SIGN_IN_BUSY,
  CONNECT_SUCCESS_ANNOUNCEMENT,
  CONNECT_TITLE,
} from "./copy.ts";

type ConnectDialogProps = {
  backend: BackendPort;
  onClose: () => void;
  onConnected: () => void;
  onAnnounce: (message: string) => void;
};

export function ConnectDialog({
  backend,
  onClose,
  onConnected,
  onAnnounce,
}: ConnectDialogProps) {
  const [config, setConfig] = useState<OAuthConfigDto | null>(null);
  const [configLoaded, setConfigLoaded] = useState(false);
  const [clientId, setClientId] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [legal, setLegal] = useState<LegalDocumentId | null>(null);

  useEffect(() => {
    let cancelled = false;

    backend
      .getOAuthConfig()
      .then((next) => {
        if (!cancelled) {
          setConfig(next);
          setClientId(next.usingCustomOverride ? (next.clientId ?? "") : "");
          setConfigLoaded(true);
        }
      })
      .catch((caught: unknown) => {
        if (!cancelled) {
          setConfigLoaded(true);
          setError(caught instanceof Error ? caught.message : CONNECT_CONFIG_LOAD_FAILED);
        }
      });

    return () => {
      cancelled = true;
    };
  }, [backend]);

  function applyConfig(next: OAuthConfigDto) {
    setConfig(next);
    setClientId(next.usingCustomOverride ? (next.clientId ?? "") : "");
  }

  async function handleSignIn() {
    setBusy(true);
    setError(null);

    try {
      if (!configLoaded) {
        setError(CONNECT_CONFIG_LOADING);
        setBusy(false);
        return;
      }

      onAnnounce(CONNECT_BROWSER_ANNOUNCEMENT);
      await backend.connectAccount();
      onAnnounce(CONNECT_SUCCESS_ANNOUNCEMENT);
      onConnected();
      onClose();
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : CONNECT_FAILED;
      setError(message);
      onAnnounce(message);
    } finally {
      setBusy(false);
    }
  }

  async function handleSaveCustom(event: FormEvent) {
    event.preventDefault();
    const trimmed = clientId.trim();
    if (trimmed.length === 0) {
      setError(CONNECT_CLIENT_ID_REQUIRED);
      onAnnounce(CONNECT_CLIENT_ID_REQUIRED);
      return;
    }
    setBusy(true);
    setError(null);
    try {
      await backend.configureOAuth(trimmed);
      const next = await backend.getOAuthConfig();
      applyConfig(next);
      onAnnounce(CONNECT_CUSTOM_SAVED);
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : CONNECT_FAILED;
      setError(message);
      onAnnounce(message);
    } finally {
      setBusy(false);
    }
  }

  async function handleResetDefault() {
    setBusy(true);
    setError(null);
    try {
      const next = await backend.resetOAuthConfig();
      applyConfig(next);
      onAnnounce(CONNECT_RESET_ANNOUNCEMENT);
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : CONNECT_FAILED;
      setError(message);
      onAnnounce(message);
    } finally {
      setBusy(false);
    }
  }

  const oauthConfigured = config?.isConfigured === true;
  const usingCustom = config?.usingCustomOverride === true;

  if (legal !== null) {
    return (
      <Dialog title={legalDocumentTitle(legal)} onClose={() => setLegal(null)} wide>
        <div className="dialog-body">
          <LegalDocument document={legal} />
          <div className="dialog-actions">
            <button type="button" className="ghost-button" onClick={() => setLegal(null)}>
              {CONNECT_BACK}
            </button>
          </div>
        </div>
      </Dialog>
    );
  }

  return (
    <Dialog title={CONNECT_TITLE} onClose={onClose} wide>
      <div className="dialog-body">
        <p>{SYSTEM_BROWSER_OAUTH_EXPLANATION}</p>
        <p>{FULL_DRIVE_SCOPE_JUSTIFICATION}</p>
        <p className="limited-use">{LIMITED_USE_SENTENCE}</p>
        <p>
          <button type="button" className="link-button" onClick={() => setLegal("privacy")}>
            Privacy Policy
          </button>
          {" · "}
          <button type="button" className="link-button" onClick={() => setLegal("limited-use")}>
            Limited Use Disclosure
          </button>
        </p>

        {!configLoaded && (
          <p role="status">{CONNECT_CONFIG_LOADING}</p>
        )}

        {configLoaded && oauthConfigured && (
          <p className="muted" role="status">
            {usingCustom ? CONNECT_READY_CUSTOM : CONNECT_READY_DEFAULT}
          </p>
        )}

        <details className="advanced-options">
          <summary>{CONNECT_ADVANCED_SUMMARY}</summary>
          <form className="advanced-options-form" onSubmit={(event) => void handleSaveCustom(event)}>
            <p className="muted">{CONNECT_ADVANCED_HELP}</p>
            <div className="field">
              <label htmlFor="oauth-client-id">{CONNECT_CLIENT_ID_LABEL}</label>
              <input
                id="oauth-client-id"
                name="clientId"
                autoComplete="off"
                value={clientId}
                onChange={(event) => setClientId(event.target.value)}
                disabled={busy}
              />
            </div>
            <div className="dialog-actions">
              <button type="submit" className="ghost-button" disabled={busy || !configLoaded}>
                {CONNECT_SAVE_CUSTOM}
              </button>
              <button
                type="button"
                className="ghost-button"
                disabled={busy || !configLoaded || !usingCustom}
                onClick={() => void handleResetDefault()}
              >
                {CONNECT_RESET_DEFAULT}
              </button>
            </div>
          </form>
        </details>

        {error !== null && (
          <p className="error" role="alert">
            {error}
          </p>
        )}

        <div className="dialog-actions">
          <button type="button" className="ghost-button" onClick={onClose} disabled={busy}>
            {CONNECT_CANCEL}
          </button>
          <button
            type="button"
            className="google-sign-in"
            disabled={busy || !configLoaded || !oauthConfigured}
            onClick={() => void handleSignIn()}
          >
            <GoogleMark />
            {busy ? CONNECT_SIGN_IN_BUSY : CONNECT_SIGN_IN}
          </button>
        </div>
      </div>
    </Dialog>
  );
}
