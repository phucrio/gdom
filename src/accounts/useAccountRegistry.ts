import { useCallback, useEffect, useRef, useState } from "react";

import type { BackendPort } from "../ipc/port.ts";
import { IPC_EVENTS, type AccountDto } from "../ipc/types.ts";
import { createLatestLoad } from "../ui/latestLoad.ts";
import { ACCOUNTS_LOAD_FAILED } from "./copy.ts";

export function useAccountRegistry(backend: BackendPort) {
  const [accounts, setAccounts] = useState<AccountDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const latest = useRef(createLatestLoad());

  const refresh = useCallback(() => {
    const token = latest.current.begin();
    setLoading(true);
    backend
      .listAccounts()
      .then((next) => {
        if (!latest.current.isCurrent(token)) {
          return;
        }
        setAccounts(next);
        setLoadError(null);
        setLoading(false);
      })
      .catch((caught: unknown) => {
        if (!latest.current.isCurrent(token)) {
          return;
        }
        setLoadError(
          caught instanceof Error
            ? caught.message
            : ACCOUNTS_LOAD_FAILED,
        );
        setLoading(false);
      });
  }, [backend]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const subscription = backend.subscribe(IPC_EVENTS.accountRegistryChanged, refresh);
    return () => {
      void subscription.then((unlisten) => {
        unlisten();
      });
    };
  }, [backend, refresh]);

  return { accounts, loading, loadError, refresh };
}
