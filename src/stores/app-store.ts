import { create } from "zustand";
import { persist } from "zustand/middleware";
import type { AdapterSnapshot } from "../types/network";

export type PageId =
  | "dashboard"
  | "adapters"
  | "ip-configuration"
  | "routing"
  | "failover"
  | "profiles"
  | "remote"
  | "diagnostics"
  | "logs"
  | "settings"
  | "about";

interface PersistedSettings {
  theme: "dark" | "light";
  refreshIntervalMs: number;
  healthProbeIntervalSeconds: number;
  closeToTray: boolean;
  setTheme: (theme: "dark" | "light") => void;
  setRefreshIntervalMs: (interval: number) => void;
  setHealthProbeIntervalSeconds: (seconds: number) => void;
  setCloseToTray: (enabled: boolean) => void;
}

interface RuntimeState {
  page: PageId;
  snapshot: AdapterSnapshot | null;
  loading: boolean;
  error: string | null;
  monitoringPaused: boolean;
  setPage: (page: PageId) => void;
  setSnapshot: (snapshot: AdapterSnapshot) => void;
  setLoading: (loading: boolean) => void;
  setError: (error: string | null) => void;
  setMonitoringPaused: (paused: boolean) => void;
}

export const useSettingsStore = create<PersistedSettings>()(
  persist(
    (set) => ({
      theme: "dark",
      refreshIntervalMs: 1000,
      healthProbeIntervalSeconds: 10,
      closeToTray: true,
      setTheme: (theme) => set({ theme }),
      setRefreshIntervalMs: (refreshIntervalMs) => set({ refreshIntervalMs }),
      setHealthProbeIntervalSeconds: (healthProbeIntervalSeconds) => set({ healthProbeIntervalSeconds }),
      setCloseToTray: (closeToTray) => set({ closeToTray }),
    }),
    { name: "charroutex-settings" },
  ),
);

export const useRuntimeStore = create<RuntimeState>((set) => ({
  page: "dashboard",
  snapshot: null,
  loading: true,
  error: null,
  monitoringPaused: false,
  setPage: (page) => set({ page }),
  setSnapshot: (snapshot) => set({ snapshot }),
  setLoading: (loading) => set({ loading }),
  setError: (error) => set({ error }),
  setMonitoringPaused: (monitoringPaused) => set({ monitoringPaused }),
}));

