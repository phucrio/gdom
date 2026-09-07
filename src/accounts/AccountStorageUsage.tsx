import { useEffect, useState } from "react";
import type { BackendPort } from "../ipc/port.ts";
import type { StorageQuotaDto } from "../ipc/types.ts";
import { formatFileSize } from "../browser/format.ts";

type StorageState = { kind: "loading" } | { kind: "error" } | { kind: "ready"; quota: StorageQuotaDto };

export function AccountStorageUsage({ accountId, connected, backend }: {
  accountId: string;
  connected: boolean;
  backend: BackendPort;
}) {
  const [state, setState] = useState<StorageState>({ kind: "loading" });
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    if (!connected) return;
    let cancelled = false;
    backend.getAccountStorage(accountId).then(
      (quota) => { if (!cancelled) setState({ kind: "ready", quota }); },
      () => { if (!cancelled) setState({ kind: "error" }); },
    );
    return () => { cancelled = true; };
  }, [accountId, connected, backend, attempt]);

  const quota = state.kind === "ready" ? state.quota : null;
  const percent = quota?.limitBytes && quota.limitBytes > 0
    ? new Intl.NumberFormat("en", { maximumFractionDigits: 1 }).format(quota.usageBytes / quota.limitBytes * 100)
    : null;
  return <section className="account-storage" aria-label="Google storage" aria-live="polite">
    <strong>Google storage</strong>
    {!connected ? <p>Reconnect to view storage.</p> : state.kind === "loading" ? <p>Loading storage…</p> : state.kind === "error" ? <>
      <p>Storage usage unavailable.</p>
      <button type="button" className="link-button" onClick={() => { setState({ kind: "loading" }); setAttempt((value) => value + 1); }}>Retry storage</button>
    </> : quota && <>
      <p>{formatFileSize(quota.usageBytes)} used{quota.limitBytes !== null ? ` of ${formatFileSize(quota.limitBytes)}` : " · No storage limit"}</p>
      {quota.limitBytes !== null && quota.limitBytes > 0 && <meter min={0} max={quota.limitBytes} value={Math.min(quota.usageBytes, quota.limitBytes)} aria-label={`${percent}% of Google storage used`} />}
      {percent !== null && <span>{percent}% used{quota.limitBytes !== null && quota.usageBytes >= quota.limitBytes ? " · Storage full" : ""}</span>}
    </>}
  </section>;
}
