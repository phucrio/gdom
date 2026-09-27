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

  const refresh = useCallback(async (): Promise<AccountDto[] | null> => {
    const token = latest.current.begin();
    setLoading(true);
    try {
      const next = await backend.listAccounts();
      if (!latest.current.isCurrent(token)) {
        return null;
      }
      setAccounts(next);
      setLoadError(null);
      setLoading(false);
      return next;
    } catch (caught: unknown) {
      if (!latest.current.isCurrent(token)) {
        return null;
      }
      setLoadError(
        caught instanceof Error
          ? caught.message
          : ACCOUNTS_LOAD_FAILED,
      );
      setLoading(false);
      return null;
    }
  }, [backend]);

  useEffect(() => {
    void refresh();
  }, [refresh]);

  useEffect(() => {
    const subscription = backend.subscribe(IPC_EVENTS.accountRegistryChanged, () => {
      void refresh();
    });
    return () => {
      void subscription.then((unlisten) => {
        unlisten();
      });
    };
  }, [backend, refresh]);

  return { accounts, loading, loadError, refresh };
}
