import {
  Activity,
  Cable,
  ChevronRight,
  CircleGauge,
  FileClock,
  FolderCog,
  Info,
  Network,
  PanelLeftClose,
  RadioTower,
  Route,
  Settings,
  ShieldCheck,
  SlidersHorizontal,
  Stethoscope,
  SunMoon,
  Wifi,
} from "lucide-react";
import { useState, type PropsWithChildren } from "react";
import { cn } from "../lib/utils";
import { type PageId, useRuntimeStore, useSettingsStore } from "../stores/app-store";
import { Button } from "./ui/button";

const navigation: Array<{ id: PageId; label: string; icon: typeof Activity; group?: string }> = [
  { id: "dashboard", label: "Dashboard", icon: CircleGauge, group: "Overview" },
  { id: "adapters", label: "Network Adapters", icon: Cable },
  { id: "ip-configuration", label: "IP Configuration", icon: SlidersHorizontal, group: "Control" },
  { id: "routing", label: "Smart Routing", icon: Route },
  { id: "failover", label: "Internet Failover", icon: RadioTower },
  { id: "profiles", label: "Network Profiles", icon: FolderCog },
  { id: "remote", label: "Remote Management", icon: ShieldCheck, group: "Tools" },
  { id: "diagnostics", label: "Diagnostics", icon: Stethoscope },
  { id: "logs", label: "Activity Logs", icon: FileClock },
  { id: "settings", label: "Settings", icon: Settings, group: "System" },
  { id: "about", label: "About", icon: Info },
];

export function AppShell({ children }: PropsWithChildren) {
  const [collapsed, setCollapsed] = useState(false);
  const { page, setPage, snapshot, monitoringPaused } = useRuntimeStore();
  const { theme, setTheme } = useSettingsStore();
  const connected = snapshot?.adapters.filter((adapter) => adapter.status === "up").length ?? 0;

  return (
    <div className={cn("min-h-screen bg-app text-foreground", theme === "light" && "light")}>
      <aside
        className={cn(
          "fixed inset-y-0 left-0 z-20 flex border-r border-white/[0.07] bg-[#07101d]/95 backdrop-blur-xl transition-[width] duration-300",
          collapsed ? "w-[76px]" : "w-[252px]",
        )}
      >
        <div className="flex min-w-0 flex-1 flex-col">
          <div className="flex h-[76px] items-center gap-3 border-b border-white/[0.06] px-5">
            <div className="relative grid h-10 w-10 shrink-0 place-items-center rounded-xl border border-cyan-300/30 bg-cyan-400/10 text-cyan-300 shadow-[0_0_32px_rgba(34,211,238,0.12)]">
              <Network size={21} />
              <span className="absolute -right-1 -top-1 h-2.5 w-2.5 rounded-full border-2 border-[#07101d] bg-emerald-400" />
            </div>
            {!collapsed && (
              <div className="min-w-0">
                <div className="truncate text-[17px] font-bold tracking-tight">CharRoute<span className="text-cyan-300">X</span></div>
                <div className="truncate text-[10px] font-medium uppercase tracking-[0.18em] text-slate-500">Your Network. Your Control.</div>
              </div>
            )}
          </div>

          <nav className="scrollbar-none flex-1 overflow-y-auto px-3 py-4">
            {navigation.map((item) => {
              const Icon = item.icon;
              return (
                <div key={item.id}>
                  {item.group && !collapsed && (
                    <div className="mb-2 mt-4 px-3 text-[10px] font-semibold uppercase tracking-[0.2em] text-slate-600 first:mt-0">{item.group}</div>
                  )}
                  {item.group && collapsed && <div className="my-2 h-px bg-white/[0.06]" />}
                  <button
                    onClick={() => setPage(item.id)}
                    title={collapsed ? item.label : undefined}
                    className={cn(
                      "group mb-1 flex h-10 w-full items-center gap-3 rounded-lg px-3 text-left text-[13px] font-medium text-slate-400 transition",
                      page === item.id
                        ? "bg-gradient-to-r from-cyan-400/15 to-blue-500/5 text-cyan-100 shadow-[inset_2px_0_0_#22d3ee]"
                        : "hover:bg-white/[0.04] hover:text-slate-200",
                    )}
                  >
                    <Icon size={17} className={cn("shrink-0", page === item.id && "text-cyan-300")} />
                    {!collapsed && <span className="truncate">{item.label}</span>}
                    {!collapsed && page === item.id && <ChevronRight size={14} className="ml-auto text-cyan-400/70" />}
                  </button>
                </div>
              );
            })}
          </nav>

          <div className="border-t border-white/[0.06] p-3">
            <div className={cn("mb-2 rounded-xl border border-white/[0.06] bg-white/[0.025] p-3", collapsed && "p-2")}>
              <div className="flex items-center gap-2.5">
                <span className="relative flex h-7 w-7 shrink-0 items-center justify-center rounded-lg bg-emerald-400/10 text-emerald-300">
                  <Wifi size={14} />
                  <span className="absolute -right-0.5 -top-0.5 h-2 w-2 animate-pulse rounded-full bg-emerald-400" />
                </span>
                {!collapsed && (
                  <div className="min-w-0">
                    <div className="text-xs font-semibold text-slate-200">{monitoringPaused ? "Monitor paused" : "Monitor active"}</div>
                    <div className="text-[10px] text-slate-500">{connected} interface{connected === 1 ? "" : "s"} linked</div>
                  </div>
                )}
              </div>
            </div>
            <Button
              aria-label="Collapse navigation"
              className="w-full border-transparent bg-transparent text-slate-500 hover:border-transparent hover:bg-white/[0.04] hover:text-slate-300"
              onClick={() => setCollapsed((value) => !value)}
            >
              <PanelLeftClose size={16} className={cn("transition", collapsed && "rotate-180")} />
              {!collapsed && <span>Collapse</span>}
            </Button>
          </div>
        </div>
      </aside>

      <main className={cn("min-h-screen transition-[margin] duration-300", collapsed ? "ml-[76px]" : "ml-[252px]")}>
        <header className="sticky top-0 z-10 flex h-[76px] items-center justify-between border-b border-white/[0.06] bg-[#07111f]/80 px-7 backdrop-blur-xl">
          <div>
            <div className="text-xs font-medium uppercase tracking-[0.14em] text-cyan-400/70">Network Command Center</div>
            <div className="mt-1 text-xs text-slate-500">Local Windows host · Protected network management</div>
          </div>
          <div className="flex items-center gap-2">
            <Button className="border-white/[0.07] bg-white/[0.03] text-slate-400" onClick={() => setTheme(theme === "dark" ? "light" : "dark")}>
              <SunMoon size={15} />
              <span className="hidden sm:inline">{theme === "dark" ? "Light" : "Dark"}</span>
            </Button>
            <div className={`ml-2 flex items-center gap-2 rounded-lg border px-3 py-2 text-xs font-medium ${monitoringPaused ? "border-amber-400/15 bg-amber-400/[0.06] text-amber-300" : "border-emerald-400/15 bg-emerald-400/[0.06] text-emerald-300"}`}>
              <Activity size={14} /> {monitoringPaused ? "Paused" : "Live"}
            </div>
          </div>
        </header>
        <div className="mx-auto max-w-[1600px] p-7">{children}</div>
      </main>
    </div>
  );
}

