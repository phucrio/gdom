import { useEffect, useState } from "react";

import type { BackendPort } from "../ipc/port.ts";
import type { OAuthConfigDto } from "../ipc/types.ts";
import {
  FULL_DRIVE_SCOPE_JUSTIFICATION,
  LIMITED_USE_SENTENCE,
  SYSTEM_BROWSER_OAUTH_EXPLANATION,
} from "../legal/copy.ts";
import { useAccountConnection } from "../accounts/useAccountConnection.ts";
import { GoogleMark } from "../accounts/GoogleMark.tsx";
import gdomIcon from "../assets/gdom-icon.svg?no-inline";

type LandingScreenProps = {
  backend: BackendPort;
  onAnnounce: (message: string) => void;
  onConnected: () => void;
  onOpenLegal: (doc: "privacy" | "limited-use") => void;
};

export function LandingScreen({
  backend,
  onAnnounce,
  onConnected,
  onOpenLegal,
}: LandingScreenProps) {
  const connection = useAccountConnection(backend, onAnnounce);
  const [config, setConfig] = useState<OAuthConfigDto | null>(null);
  const [configLoaded, setConfigLoaded] = useState(false);
  const signingIn = connection.busy;
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelled = false;
    backend
      .getOAuthConfig()
      .then((cfg) => {
        if (!cancelled) {
          setConfig(cfg);
          setConfigLoaded(true);
        }
      })
      .catch((err: unknown) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : "Failed to load OAuth configuration.");
          setConfigLoaded(true);
        }
      });
    return () => {
      cancelled = true;
    };
  }, [backend]);

  async function handleSignIn() {
    if (!config?.canSignIn) {
      setError(
        "Google Desktop OAuth client secret is missing. Set GDOM_GOOGLE_CLIENT_SECRET before starting the app.",
      );
      return;
    }
    setError(null);
    onAnnounce("Opening Google sign-in in the system browser.");
    try {
      const account = await connection.connect();
      if (!account) return;
      onAnnounce("Sign in successful.");
      onConnected();
    } catch (caught: unknown) {
      const msg = caught instanceof Error ? caught.message : "Sign in failed.";
      setError(msg);
      onAnnounce(msg);
    }
  }

  async function handleCancel() {
    try {
      await connection.cancel();
      setError(null);
    } catch (caught: unknown) {
      setError(caught instanceof Error ? caught.message : "Could not cancel sign-in.");
    }
  }

  return (
    <div className="landing-screen" role="region" aria-label="Sign in to GDOM">
      <div className="landing-card">
        <div className="landing-header">
          <img src={gdomIcon} width={48} height={48} alt="" className="landing-logo" />
          <h1 className="landing-title">GDOM</h1>
          <p className="landing-subtitle">Google Drive Owner Migrator</p>
        </div>

        <p className="landing-desc">
          Safely and recursively transfer Google Drive file and folder ownership between personal Gmail accounts.
        </p>

        <div className="landing-scope-box" role="note">
          <p className="scope-box-title">Why Drive permissions are required</p>
          <p className="scope-box-text">{FULL_DRIVE_SCOPE_JUSTIFICATION}</p>
          <p className="scope-box-text">{SYSTEM_BROWSER_OAUTH_EXPLANATION}</p>
          <p className="scope-box-text scope-box-limited">{LIMITED_USE_SENTENCE}</p>
        </div>

        {error && (
          <div className="landing-error" role="alert">
            <p>{error}</p>
          </div>
        )}

        {!configLoaded ? (
          <p className="landing-status" role="status">Checking configuration…</p>
        ) : !config?.canSignIn ? (
          <div className="config-guidance" role="alert">
            <p className="config-guidance-title">OAuth Configuration Required</p>
            <p className="config-guidance-body">
              Local testing requires <code>GDOM_GOOGLE_CLIENT_SECRET</code> to be set in your terminal environment before running the app. Ensure Google Drive API is enabled in your Google Cloud Console.
            </p>
          </div>
        ) : (
          <div className="landing-actions">
            <button
              type="button"
              className="primary-button landing-cta"
              onClick={() => void handleSignIn()}
              disabled={signingIn || connection.cancelling}
              aria-busy={signingIn}
            >
              <GoogleMark />
              <span>{signingIn ? "Opening system browser…" : "Sign in with Google"}</span>
            </button>
            {(signingIn || connection.cancelling) && (
              <button type="button" className="ghost-button" disabled={connection.cancelling} onClick={() => void handleCancel()}>
                {connection.cancelling ? "Cancelling…" : "Cancel sign-in"}
              </button>
            )}
          </div>
        )}

        <div className="landing-legal">
          <button type="button" className="link-button" onClick={() => onOpenLegal("privacy")}>
            Privacy Policy
          </button>
          <span aria-hidden="true">·</span>
          <button type="button" className="link-button" onClick={() => onOpenLegal("limited-use")}>
            Limited Use Disclosure
          </button>
        </div>
      </div>
    </div>
  );
}
