import { QueryClient, QueryClientProvider } from "@tanstack/react-query";
import { useEffect } from "react";
import { RouterProvider } from "react-router-dom";
import { backend } from "../lib/tauri";
import { router } from "../routes/router";

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      staleTime: 30_000,
      retry: 1,
      refetchOnWindowFocus: false,
    },
  },
});

export function App() {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let unlistenTimeline: (() => void) | undefined;
    const relevant = new Set(["home", "home-shell", "champions", "champion-profile", "matches", "career"]);
    const refreshArchiveViews = () => queryClient.invalidateQueries({ predicate: ({ queryKey }) => relevant.has(String(queryKey[0])) });
    void backend.onSyncStateChanged((state) => {
      queryClient.setQueryData(["sync-state"], state);
      queryClient.setQueryData(["home"], (previous: { syncState?: unknown } | undefined) => previous ? { ...previous, syncState: state } : previous);
      if (state.status === "success") {
        void refreshArchiveViews();
      }
    }).then((cleanup) => { if (disposed) cleanup(); else unlisten = cleanup; }).catch(() => undefined);
    void backend.onTimelineFactsChanged(() => { void refreshArchiveViews(); })
      .then((cleanup) => { if (disposed) cleanup(); else unlistenTimeline = cleanup; }).catch(() => undefined);
    const request = (trigger: "periodic" | "resume") => { void backend.requestFreshnessCheck(trigger).catch(() => undefined); };
    const interval = window.setInterval(() => request("periodic"), 5 * 60_000);
    const resume = () => request("resume");
    window.addEventListener("focus", resume);
    return () => { disposed = true; window.clearInterval(interval); window.removeEventListener("focus", resume); unlisten?.(); unlistenTimeline?.(); };
  }, []);
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
