import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import { AppShell } from "./components/app-shell";
import { useAdapterMonitor } from "./hooks/use-adapter-monitor";
import { AboutPage } from "./pages/about";
import { Adapters } from "./pages/adapters";
import { Dashboard } from "./pages/dashboard";
import { DiagnosticsPage } from "./pages/diagnostics";
import { IpConfigurationPage } from "./pages/ip-configuration";
import { ProfilesPage } from "./pages/profiles";
import { ActivityLogsPage } from "./pages/activity-logs";
import { RoutingPage } from "./pages/routing";
import { SettingsPage } from "./pages/settings";
import { FailoverPage } from "./pages/failover";
import { RemoteManagementPage } from "./pages/remote";
import { useRuntimeStore, useSettingsStore } from "./stores/app-store";
import { getMonitoringPaused, setCloseToTray, setHealthProbeInterval } from "./services/network";

export default function App() {
  const page = useRuntimeStore((state) => state.page);
  const theme = useSettingsStore((state) => state.theme);
  const closeToTray = useSettingsStore((state) => state.closeToTray);
  const healthProbeIntervalSeconds = useSettingsStore((state) => state.healthProbeIntervalSeconds);
  const setMonitoringPaused = useRuntimeStore((state) => state.setMonitoringPaused);
  const refresh = useAdapterMonitor();
  useEffect(() => { document.documentElement.style.colorScheme = theme; }, [theme]);
  useEffect(() => { void setCloseToTray(closeToTray); }, [closeToTray]);
  useEffect(() => { void setHealthProbeInterval(healthProbeIntervalSeconds); }, [healthProbeIntervalSeconds]);
  useEffect(() => {
    void getMonitoringPaused().then(setMonitoringPaused);
    const unlisten = listen<boolean>("monitoring-state-changed", (event) => setMonitoringPaused(event.payload));
    return () => { void unlisten.then((dispose) => dispose()); };
  }, [setMonitoringPaused]);

  const content = (() => {
    switch (page) {
      case "dashboard": return <Dashboard onRefresh={() => void refresh()} />;
      case "adapters": return <Adapters onRefresh={() => void refresh()} />;
      case "ip-configuration": return <IpConfigurationPage />;
      case "routing": return <RoutingPage />;
      case "failover": return <FailoverPage />;
      case "profiles": return <ProfilesPage />;
      case "remote": return <RemoteManagementPage />;
      case "diagnostics": return <DiagnosticsPage />;
      case "logs": return <ActivityLogsPage />;
      case "settings": return <SettingsPage />;
      case "about": return <AboutPage />;
      default: return <Dashboard onRefresh={() => void refresh()} />;
    }
  })();

  return <AppShell>{content}</AppShell>;
}

