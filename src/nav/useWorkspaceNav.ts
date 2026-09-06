import { useCallback, useEffect, useState } from "react";

import { hashForView, workspaceViewFromHash, type WorkspaceView } from "./workspace.ts";

export function useWorkspaceNav() {
  const [view, setView] = useState<WorkspaceView>(() =>
    workspaceViewFromHash(window.location.hash),
  );

  const goTo = useCallback((next: WorkspaceView) => {
    setView(next);
    const hash = hashForView(next);
    if (window.location.hash !== hash) {
      window.location.hash = hash;
    }
  }, []);

  useEffect(() => {
    function onHashChange() {
      setView(workspaceViewFromHash(window.location.hash));
    }
    window.addEventListener("hashchange", onHashChange);
    return () => {
      window.removeEventListener("hashchange", onHashChange);
    };
  }, []);

  return { view, goTo };
}
