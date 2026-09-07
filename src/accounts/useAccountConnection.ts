import { useEffect, useRef, useState } from "react";
import type { BackendPort } from "../ipc/port.ts";

type ConnectionAttempt = {
  id: string;
  registration: Promise<void>;
  cancelled: boolean;
};

export function useAccountConnection(backend: BackendPort, onAnnounce: (message: string) => void) {
  const announce = useRef(onAnnounce);
  announce.current = onAnnounce;
  const activeAttempt = useRef<ConnectionAttempt | null>(null);
  const [busy, setBusy] = useState(false);
  const [cancelling, setCancelling] = useState(false);

  useEffect(() => () => {
    const attempt = activeAttempt.current;
    if (attempt) {
      attempt.cancelled = true;
      void attempt.registration.then(
        () => backend.cancelAccountConnection(attempt.id),
        () => undefined,
      ).catch((error: unknown) => {
        announce.current(error instanceof Error ? error.message : "Could not cancel sign-in.");
      });
    }
  }, [backend]);

  async function connect() {
    if (activeAttempt.current) throw new Error("Cancel the previous sign-in before trying again.");
    let finished = false;
    const id = crypto.randomUUID();
    const attempt: ConnectionAttempt = {
      id,
      registration: backend.beginAccountConnection(id),
      cancelled: false,
    };
    activeAttempt.current = attempt;
    setBusy(true);
    try {
      await attempt.registration;
      if (attempt.cancelled) return undefined;
      const account = await backend.connectAccount(id);
      finished = true;
      return attempt.cancelled ? undefined : account;
    } catch (error: unknown) {
      if (!attempt.cancelled) {
        await backend.cancelAccountConnection(id);
        finished = true;
        throw error;
      }
      return undefined;
    } finally {
      if (finished && activeAttempt.current === attempt && !attempt.cancelled) {
        activeAttempt.current = null;
        setBusy(false);
      }
    }
  }

  async function cancel() {
    const attempt = activeAttempt.current;
    if (!attempt) return;
    attempt.cancelled = true;
    setCancelling(true);
    try {
      await attempt.registration.then(
        () => backend.cancelAccountConnection(attempt.id),
        () => undefined,
      );
      if (activeAttempt.current === attempt) {
        activeAttempt.current = null;
        setBusy(false);
      }
    } finally {
      setCancelling(false);
    }
  }

  return { connect, cancel, cancelling, busy };
}
