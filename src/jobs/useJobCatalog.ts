import { useCallback, useEffect, useRef, useState } from "react";

import type { BackendPort } from "../ipc/port.ts";
import { IPC_EVENTS, type JobDto } from "../ipc/types.ts";
import { createLatestLoad } from "../ui/latestLoad.ts";
import { JOBS_LOAD_FAILED } from "./copy.ts";

const JOB_CATALOG_EVENTS = [
  IPC_EVENTS.jobListChanged,
  IPC_EVENTS.jobStatusChanged,
  IPC_EVENTS.migrationCompleted,
] as const;

export function useJobCatalog(backend: BackendPort) {
  const [jobs, setJobs] = useState<JobDto[]>([]);
  const [loading, setLoading] = useState(true);
  const [loadError, setLoadError] = useState<string | null>(null);
  const latest = useRef(createLatestLoad());

  const refresh = useCallback(() => {
    const token = latest.current.begin();
    setLoading(true);
    backend
      .listJobs()
      .then((next) => {
        if (!latest.current.isCurrent(token)) {
          return;
        }
        setJobs(next);
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
            : JOBS_LOAD_FAILED,
        );
        setLoading(false);
      });
  }, [backend]);

  useEffect(() => {
    refresh();
  }, [refresh]);

  useEffect(() => {
    const subscriptions = JOB_CATALOG_EVENTS.map((event) => backend.subscribe(event, refresh));
    return () => {
      void Promise.all(subscriptions).then((unlistens) => {
        for (const unlisten of unlistens) {
          unlisten();
        }
      });
    };
  }, [backend, refresh]);

  return { jobs, loading, loadError, setLoadError, refresh };
}
