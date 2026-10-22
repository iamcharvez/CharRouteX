import { useCallback, useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { getNetworkAdapters } from "../services/network";
import { useRuntimeStore, useSettingsStore } from "../stores/app-store";

function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export function useAdapterMonitor() {
  const refreshIntervalMs = useSettingsStore((state) => state.refreshIntervalMs);
  const { setSnapshot, setLoading, setError } = useRuntimeStore();

  const refresh = useCallback(async () => {
    try {
      const snapshot = await getNetworkAdapters();
      setSnapshot(snapshot);
      setError(null);
    } catch (error) {
      setError(errorMessage(error));
    } finally {
      setLoading(false);
    }
  }, [setError, setLoading, setSnapshot]);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), refreshIntervalMs);
    return () => window.clearInterval(timer);
  }, [refresh, refreshIntervalMs]);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let debounceTimer: number | undefined;
    void listen("network-changed", () => {
      window.clearTimeout(debounceTimer);
      debounceTimer = window.setTimeout(() => void refresh(), 250);
    }).then((cleanup) => {
      if (disposed) cleanup();
      else unlisten = cleanup;
    });
    return () => {
      disposed = true;
      window.clearTimeout(debounceTimer);
      unlisten?.();
    };
  }, [refresh]);

  return refresh;
}

