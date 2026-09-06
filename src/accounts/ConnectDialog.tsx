import { useEffect, useState } from "react";

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
  CONNECT_CONFIG_LOAD_FAILED,
  CONNECT_CONFIG_LOADING,
  CONNECT_FAILED,
  CONNECT_IMPORT_JSON,
  CONNECT_IMPORT_SAVED,
  CONNECT_READY_CUSTOM,
  CONNECT_READY_DEFAULT,
  CONNECT_RESET_ANNOUNCEMENT,
  CONNECT_RESET_DEFAULT,
  CONNECT_SECRET_REQUIRED,
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

  async function handleSignIn() {
    setBusy(true);
    setError(null);

    try {
      if (!configLoaded) {
        setError(CONNECT_CONFIG_LOADING);
        setBusy(false);
        return;
      }

      if (config?.canSignIn !== true) {
        setError(CONNECT_SECRET_REQUIRED);
        onAnnounce(CONNECT_SECRET_REQUIRED);
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

  async function handleImportJson() {
    setBusy(true);
    setError(null);
    try {
      const previousCanSignIn = config?.canSignIn === true;
      const previousClientId = config?.clientId;
      const next = await backend.importOAuthClient();
      setConfig(next);
      if (
        next.canSignIn &&
        next.usingCustomOverride &&
        (next.clientId !== previousClientId || !previousCanSignIn)
      ) {
        onAnnounce(CONNECT_IMPORT_SAVED);
      }
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
      setConfig(next);
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
  const canSignIn = config?.canSignIn === true;

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

        {configLoaded && oauthConfigured && canSignIn && (
          <p className="muted" role="status">
            {usingCustom ? CONNECT_READY_CUSTOM : CONNECT_READY_DEFAULT}
          </p>
        )}

        {configLoaded && !canSignIn && (
          <p className="error" role="status">
            {CONNECT_SECRET_REQUIRED}
          </p>
        )}

        <details className="advanced-options">
          <summary>{CONNECT_ADVANCED_SUMMARY}</summary>
          <div className="advanced-options-form">
            <p className="muted">{CONNECT_ADVANCED_HELP}</p>
            {usingCustom && config?.clientId ? (
              <p className="muted">Using client ID {config.clientId}</p>
            ) : null}
            <div className="dialog-actions">
              <button
                type="button"
                className="ghost-button"
                disabled={busy || !configLoaded}
                onClick={() => void handleImportJson()}
              >
                {CONNECT_IMPORT_JSON}
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
          </div>
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
            disabled={busy || !configLoaded || !oauthConfigured || !canSignIn}
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
