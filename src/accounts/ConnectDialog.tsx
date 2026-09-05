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
          setConfigLoaded(true);
        }
      })
      .catch((caught: unknown) => {
        if (!cancelled) {
          setConfigLoaded(true);
          setError(caught instanceof Error ? caught.message : "Could not read OAuth configuration.");
        }
      });

    return () => {
      cancelled = true;
    };
  }, [backend]);

  async function handleConnect(event: FormEvent) {
    event.preventDefault();
    setBusy(true);
    setError(null);

    try {
      if (!configLoaded) {
        setError("OAuth configuration is still loading.");
        setBusy(false);
        return;
      }
      const configured = config?.isConfigured === true;
      if (!configured) {
        const trimmed = clientId.trim();
        if (trimmed.length === 0) {
          setError("Enter a Google Cloud OAuth client ID before connecting.");
          setBusy(false);
          return;
        }
        await backend.configureOAuth(trimmed);
      }

      onAnnounce("Opening Google sign-in in the system browser.");
      await backend.connectAccount();
      onAnnounce("Account connected.");
      onConnected();
      onClose();
    } catch (caught) {
      const message = caught instanceof Error ? caught.message : "Could not connect the account.";
      setError(message);
      onAnnounce(message);
    } finally {
      setBusy(false);
    }
  }

  const oauthConfigured = config?.isConfigured === true;

  if (legal !== null) {
    return (
      <Dialog title={legalDocumentTitle(legal)} onClose={() => setLegal(null)} wide>
        <div className="dialog-body">
          <LegalDocument document={legal} />
          <div className="dialog-actions">
            <button type="button" className="ghost-button" onClick={() => setLegal(null)}>
              Back
            </button>
          </div>
        </div>
      </Dialog>
    );
  }

  return (
    <Dialog title="Connect a Google account" onClose={onClose} wide>
      <form className="dialog-body" onSubmit={handleConnect}>
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

        {!configLoaded && <p role="status">Checking OAuth configuration…</p>}

        {configLoaded && !oauthConfigured && (
          <div className="field">
            <p className="notice" role="status">
              OAuth is not configured. Enter the Google Cloud OAuth client ID for a desktop app.
              GDOM stores the client ID locally. It never asks for or displays a client secret.
            </p>
            <label htmlFor="oauth-client-id">OAuth client ID</label>
            <input
              id="oauth-client-id"
              name="clientId"
              autoComplete="off"
              value={clientId}
              onChange={(event) => setClientId(event.target.value)}
              disabled={busy}
            />
          </div>
        )}

        {oauthConfigured && config !== null && config.clientId !== null && (
          <p className="muted">Using OAuth client ID {config.clientId}.</p>
        )}

        {error !== null && (
          <p className="error" role="alert">
            {error}
          </p>
        )}

        <div className="dialog-actions">
          <button type="button" className="ghost-button" onClick={onClose} disabled={busy}>
            Cancel
          </button>
          <button type="submit" className="primary-button" disabled={busy || !configLoaded}>
            {busy ? "Waiting for browser…" : "Connect"}
          </button>
        </div>
      </form>
    </Dialog>
  );
}
