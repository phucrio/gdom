import { useCallback, useEffect, useRef, useState } from "react";
import type { BackendPort } from "../ipc/port.ts";
import type { UpdateStatusDto } from "../ipc/types.ts";

export type UpdateBackend = Pick<BackendPort,
  "getUpdateStatus" | "checkForUpdates" | "downloadUpdate" | "installUpdate">;
const POLL_INTERVAL_MS = 500;

export function useUpdates(backend: UpdateBackend) {
  const [status, setStatus] = useState<UpdateStatusDto | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pending, setPending] = useState(true);
  const operationActive = useRef(false);
  const startup = useRef<Promise<{ status: UpdateStatusDto; error: string | null }> | null>(null);
  const generation = useRef(0);

  useEffect(() => {
    let active = true;
    startup.current ??= backend.getUpdateStatus().then(async (current) => {
      try {
        return { status: current.phase === "idle" ? await backend.checkForUpdates() : current, error: null };
      } catch (failure: unknown) {
        return { status: current, error: failure instanceof Error ? failure.message : "Unable to check for updates." };
      }
    });
    void startup.current.then((result) => {
      if (active) { setStatus(result.status); setError(result.error); }
    }).catch((failure: unknown) => {
      if (active) setError(failure instanceof Error ? failure.message : "Unable to check for updates.");
    }).finally(() => { if (active) setPending(false); });
    return () => { active = false; };
  }, [backend]);

  const run = useCallback(async (operation: () => Promise<UpdateStatusDto>) => {
    if (operationActive.current) return;
    operationActive.current = true;
    generation.current += 1;
    setPending(true);
    setError(null);
    try {
      setStatus(await operation());
    } catch (failure: unknown) {
      setError(failure instanceof Error ? failure.message : "The update could not be completed. Try again.");
    } finally {
      generation.current += 1;
      operationActive.current = false;
      setPending(false);
    }
  }, []);

  useEffect(() => {
    if (!pending && status?.phase !== "downloading" && status?.phase !== "installing" && status?.phase !== "checking") return;
    let active = true;
    let timer: ReturnType<typeof setTimeout>;
    async function poll() {
      const expectedGeneration = generation.current;
      try {
        const current = await backend.getUpdateStatus();
        if (active && expectedGeneration === generation.current) setStatus(current);
      } catch (failure: unknown) {
        if (active) setError(failure instanceof Error ? failure.message : "Unable to read update progress.");
      }
      if (active) timer = setTimeout(() => { void poll(); }, POLL_INTERVAL_MS);
    }
    timer = setTimeout(() => { void poll(); }, POLL_INTERVAL_MS);
    return () => { active = false; clearTimeout(timer); };
  }, [backend, pending, status?.phase]);

  return {
    status, error, pending,
    check: () => run(() => backend.checkForUpdates()),
    download: () => run(() => backend.downloadUpdate({ confirmed: true })),
    install: () => run(() => backend.installUpdate({ confirmed: true })),
  };
}
