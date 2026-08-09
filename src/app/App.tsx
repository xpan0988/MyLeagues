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
    void backend.onSyncStateChanged((state) => {
      queryClient.setQueryData(["sync-state"], state);
      if (state.status === "success") {
        const refreshed = new Set(["home", "home-shell", "champions", "champion-profile", "matches", "career"]);
        void queryClient.invalidateQueries({ predicate: ({ queryKey }) => refreshed.has(String(queryKey[0])) });
      }
    }).then((cleanup) => { if (disposed) cleanup(); else unlisten = cleanup; }).catch(() => undefined);
    return () => { disposed = true; unlisten?.(); };
  }, []);
  return (
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>
  );
}
