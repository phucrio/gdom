import { useEffect, useState } from "react";
import type { BackendPort } from "../ipc/port.ts";
import { IPC_EVENTS, type JobDto, type JobItemDto } from "../ipc/types.ts";

const JOB_EVENTS = [IPC_EVENTS.scanProgress, IPC_EVENTS.migrationProgress,
  IPC_EVENTS.itemStateChanged, IPC_EVENTS.jobStatusChanged,
  IPC_EVENTS.canaryCompleted, IPC_EVENTS.migrationCompleted] as const;

export function useProgressSnapshot(backend: BackendPort, jobId: string, pages: number) {
  const [job, setJob] = useState<JobDto | null>(null);
  const [items, setItems] = useState<JobItemDto[]>([]);
  const [hasMore, setHasMore] = useState(false);
  const [loadingItems, setLoadingItems] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [revision, setRevision] = useState(0);

  useEffect(() => {
    let disposed = false;
    let loading = false;
    let refreshPending = false;
    const unlisteners: Array<() => void> = [];

    async function refresh() {
      if (disposed) return;
      if (loading) { refreshPending = true; return; }
      loading = true;
      setLoadingItems(pages > 0);
      try {
        do {
          refreshPending = false;
          try {
            const [nextJob, itemPages] = await Promise.all([
              backend.getJob(jobId),
              Promise.all(Array.from({ length: pages }, (_, index) =>
                backend.listJobItems(jobId, null, index + 1))),
            ]);
            if (disposed) return;
            setJob(nextJob);
            setItems(itemPages.flatMap((page) => page.items));
            const lastPage = itemPages[itemPages.length - 1];
            setHasMore(lastPage !== undefined && lastPage.page * lastPage.pageSize < lastPage.total);
            setLoadError(null);
          } catch (caught: unknown) {
            if (!disposed) setLoadError(caught instanceof Error ? caught.message : "Failed to load migration progress.");
          }
        } while (refreshPending && !disposed);
      } finally {
        loading = false;
        if (!disposed) setLoadingItems(false);
      }
    }

    for (const event of JOB_EVENTS) {
      void backend.subscribe(event, () => void refresh()).then((unlisten) => {
        if (disposed) unlisten();
        else { unlisteners.push(unlisten); void refresh(); }
      }).catch((caught: unknown) => {
        if (!disposed) setLoadError(caught instanceof Error ? caught.message : "Failed to subscribe to progress updates.");
      });
    }
    void refresh();
    return () => { disposed = true; unlisteners.forEach((unlisten) => unlisten()); };
  }, [backend, jobId, pages, revision]);

  return { job, items, hasMore, loadingItems, loadError,
    refresh: () => setRevision((value) => value + 1) };
}
