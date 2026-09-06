import { useEffect, useRef, useState } from "react";

import { isCommandMissing, toIpcError } from "../ipc/errors.ts";
import type { BackendPort } from "../ipc/port.ts";
import { IPC_EVENTS, type JobItemsPage } from "../ipc/types.ts";
import { createLatestLoad } from "../ui/latestLoad.ts";
import { DRY_RUN_ITEMS_UNAVAILABLE } from "./copy.ts";
import { backendItemFilter, type JobItemFilter } from "./filters.ts";
import { emptyItemsPage } from "./items.ts";

const ITEM_EVENTS = [
  IPC_EVENTS.scanProgress,
  IPC_EVENTS.itemStateChanged,
  IPC_EVENTS.jobStatusChanged,
] as const;

type ItemBackend = Pick<BackendPort, "listJobItems" | "subscribe">;

export function useJobItems(
  backend: ItemBackend,
  jobId: string | null,
  filter: JobItemFilter,
  page: number,
) {
  const [result, setResult] = useState<JobItemsPage>(emptyItemsPage());
  const [loading, setLoading] = useState(false);
  const [unavailable, setUnavailable] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const latest = useRef(createLatestLoad());
  const jobIdRef = useRef(jobId);
  const filterRef = useRef(filter);
  const pageRef = useRef(page);
  const backendRef = useRef(backend);
  jobIdRef.current = jobId;
  filterRef.current = filter;
  pageRef.current = page;
  backendRef.current = backend;

  useEffect(() => {
    if (jobId === null) {
      setResult(emptyItemsPage());
      setLoading(false);
      setUnavailable(false);
      setError(null);
      return;
    }

    const token = latest.current.begin();
    setLoading(true);
    backend
      .listJobItems(jobId, backendItemFilter(filter), page)
      .then((next) => {
        if (!latest.current.isCurrent(token)) {
          return;
        }
        setResult(next);
        setUnavailable(false);
        setError(null);
        setLoading(false);
      })
      .catch((caught: unknown) => {
        if (!latest.current.isCurrent(token)) {
          return;
        }
        const ipcError = toIpcError(caught, "list_job_items");
        if (isCommandMissing(ipcError)) {
          setUnavailable(true);
          setError(DRY_RUN_ITEMS_UNAVAILABLE);
          setResult(emptyItemsPage());
          setLoading(false);
          return;
        }
        setError(caught instanceof Error ? caught.message : DRY_RUN_ITEMS_UNAVAILABLE);
        setLoading(false);
      });
  }, [backend, filter, jobId, page]);

  useEffect(() => {
    if (jobId === null) {
      return;
    }
    const refresh = () => {
      const currentJobId = jobIdRef.current;
      if (currentJobId === null) {
        return;
      }
      const token = latest.current.begin();
      backendRef.current
        .listJobItems(currentJobId, backendItemFilter(filterRef.current), pageRef.current)
        .then((next) => {
          if (!latest.current.isCurrent(token)) {
            return;
          }
          setResult(next);
          setError(null);
        })
        .catch(() => {
          /* Keep the last successful page while a refresh fails. */
        });
    };
    const subscriptions = ITEM_EVENTS.map((event) => backend.subscribe(event, refresh));
    return () => {
      void Promise.all(subscriptions).then((unlistens) => {
        for (const unlisten of unlistens) {
          unlisten();
        }
      });
    };
  }, [backend, jobId]);

  return { page: result, loading, unavailable, error };
}
