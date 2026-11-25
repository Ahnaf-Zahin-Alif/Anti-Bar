import { useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AppWindow,
  BatteryCharging,
  Bluetooth,
  FolderDown,
  Lock,
  Power,
  Search,
  Settings,
  SlidersHorizontal,
  Trash2,
  Volume2,
  VolumeX,
  Wifi,
} from "lucide-react";
import {
  AppConfig,
  AudioSessionInfo,
  DownloadEntry,
  RunningWindow,
  SystemStatus,
  TrashInfo,
  WifiNetwork,
} from "../types";
import { resolveLang, STRINGS } from "../i18n";

type PopoverKind = "none" | "downloads" | "mixer" | "control";

export function DockWindow() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [windows, setWindows] = useState<RunningWindow[]>([]);
  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);
  const [fullscreenHidden, setFullscreenHidden] = useState(false);
  const [popover, setPopover] = useState<PopoverKind>("none");
  const [downloads, setDownloads] = useState<DownloadEntry[]>([]);
  const [audioSessions, setAudioSessions] = useState<AudioSessionInfo[]>([]);
  const [wifiNets, setWifiNets] = useState<WifiNetwork[]>([]);
  const [sysStatus, setSysStatus] = useState<SystemStatus>({
    batteryPercent: 100,
    acOnline: true,
    hasBattery: false,
  });
  const [trash, setTrash] = useState<TrashInfo>({ items: 0, sizeBytes: 0 });

  useEffect(() => {
    invoke<AppConfig>("get_config").then(setConfig).catch(console.error);
    const refresh = () => {
      invoke<RunningWindow[]>("get_running_windows").then(setWindows).catch(() => {});
      invoke<TrashInfo>("get_trash_info").then(setTrash).catch(() => {});
      invoke<SystemStatus>("get_system_status").then(setSysStatus).catch(() => {});
    };
    refresh();
    const timer = setInterval(refresh, 1600);
    const unlistenCfg = listen<AppConfig>("antibar://config-changed", (e) => setConfig(e.payload));
    const unlistenFs = listen<boolean>("antibar://fullscreen-changed", (e) => setFullscreenHidden(e.payload));
    return () => {
      clearInterval(timer);
      unlistenCfg.then((u) => u());
      unlistenFs.then((u) => u());
    };
  }, []);

  const lang = resolveLang(config?.language ?? "auto");
  const t = STRINGS[lang];

  const items = useMemo(() => {
    if (!config) return [];
    const pinned = config.pinnedApps.map((p) => {
      const match = windows.find(
        (w) =>
          w.exeName.toLowerCase() === p.path.toLowerCase() ||
          w.exePath.toLowerCase() === p.path.toLowerCase()
      );
      return {
        key: `pin:${p.id}`,
        name: p.name,
        path: p.path,
        args: p.args ?? null,
        hwnd: match?.hwnd ?? null,
        isFocused: match?.isFocused ?? false,
        icon: match?.iconDataUrl ?? p.iconDataUrl ?? null,
      };
    });

    const unpinnedRunning = windows
      .filter(
        (w) =>
          !config.pinnedApps.some(
            (p) =>
              w.exeName.toLowerCase() === p.path.toLowerCase() ||
              w.exePath.toLowerCase() === p.path.toLowerCase()
          )
      )
      .slice(0, 7)
      .map((w) => ({
        key: `win:${w.hwnd}`,
        name: w.title,
        path: w.exePath,
        args: null,
        hwnd: w.hwnd,
        isFocused: w.isFocused,
        icon: w.iconDataUrl ?? null,
      }));

    return [...pinned, ...unpinnedRunning];
  }, [config, windows]);

  if (!config) return null;
  if (config.hideInFullscreen && fullscreenHidden) return null;

  const iconSize = config.iconSize || 48;
  const maxScale = config.magnification || 1.38;

  const calcScale = (idx: number) => {
    if (hoveredIdx === null) return 1;
    const dist = Math.abs(idx - hoveredIdx);
    if (dist === 0) return maxScale;
    if (dist === 1) return 1 + (maxScale - 1) * 0.52;
    if (dist === 2) return 1 + (maxScale - 1) * 0.18;
    return 1;
  };

  const togglePopover = async (next: PopoverKind) => {
    const resolved: PopoverKind = popover === next ? "none" : next;
    setPopover(resolved);
    invoke("resize_overlay", {
      label: "dock",
      width: 860,
      height: resolved === "none" ? 118 : 390,
    }).catch(() => {});
    if (resolved === "downloads") {
      setDownloads(await invoke<DownloadEntry[]>("get_downloads"));
    } else if (resolved === "mixer") {
      setAudioSessions(await invoke<AudioSessionInfo[]>("get_audio_sessions"));
    } else if (resolved === "control") {
      setWifiNets(await invoke<WifiNetwork[]>("get_wifi_networks"));
      setSysStatus(await invoke<SystemStatus>("get_system_status"));
    }
  };

  const updateVolume = async (sess: AudioSessionInfo, vol: number, muted: boolean) => {
    setAudioSessions((prev) =>
      prev.map((s) => (s.pid === sess.pid && s.isMaster === sess.isMaster ? { ...s, volume: vol, muted } : s))
    );
    await invoke("set_app_volume", { pid: sess.pid, isMaster: sess.isMaster, volume: vol, muted });
  };

  return (
    <div className="dock-shell">
      {popover === "downloads" && (
        <div className="dock-popover" style={{ right: 56 }}>
          <div className="dock-popover-head">
            <span>{t.downloads}</span>
            <button
              className="icon-btn"
              onClick={() =>
                invoke("update_config", {
                  newConfig: {
                    ...config,
                    downloadsView: config.downloadsView === "grid" ? "list" : "grid",
                  },
                })
              }
            >
              {config.downloadsView === "grid" ? "List" : "Grid"}
            </button>
          </div>
          <div className={config.downloadsView === "grid" ? "downloads-grid" : "downloads-list"}>
            {downloads.map((d) => (
              <button
                key={d.path}
                className="dl-entry"
                onClick={() => invoke("launch_app", { path: d.path, args: null })}
                title={d.name}
              >
                <FolderDown size={14} />
                <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                  {d.name}
                </span>
              </button>
            ))}
          </div>
        </div>
      )}

      {popover === "mixer" && (
        <div className="dock-popover" style={{ right: 48 }}>
          <div className="dock-popover-head">
            <span>Volume Mixer</span>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {audioSessions.map((s) => (
              <div key={`${s.isMaster}-${s.pid}`} style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <button className="icon-btn" onClick={() => updateVolume(s, s.volume, !s.muted)}>
                  {s.muted ? <VolumeX size={14} /> : <Volume2 size={14} />}
                </button>
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 12, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {s.name}
                  </div>
                  <input
                    type="range"
                    min={0}
                    max={1}
                    step={0.02}
                    value={s.volume}
                    onChange={(e) => updateVolume(s, Number(e.target.value), s.muted)}
                    style={{ width: "100%" }}
                  />
                </div>
                <span style={{ fontSize: 11, width: 32, textAlign: "right" }}>
                  {Math.round(s.volume * 100)}%
                </span>
              </div>
            ))}
          </div>
        </div>
      )}

      {popover === "control" && (
        <div className="dock-popover" style={{ right: 32 }}>
          <div className="dock-popover-head">
            <span>Control Center</span>
            {sysStatus.hasBattery && (
              <span style={{ fontSize: 12, color: "#38bdf8", display: "inline-flex", alignItems: "center", gap: 4 }}>
                <BatteryCharging size={14} /> {sysStatus.batteryPercent}%
              </span>
            )}
          </div>
          <div style={{ display: "flex", gap: 6, marginBottom: 10 }}>
            <button className="icon-btn" style={{ flex: 1, gap: 6 }} onClick={() => invoke("run_power_action", { action: "wifi" })}>
              <Wifi size={14} /> Wi-Fi
            </button>
            <button className="icon-btn" style={{ flex: 1, gap: 6 }} onClick={() => invoke("run_power_action", { action: "bluetooth" })}>
              <Bluetooth size={14} /> Bluetooth
            </button>
            <button className="icon-btn" onClick={() => invoke("run_power_action", { action: "lock" })} title="Lock PC">
              <Lock size={14} />
            </button>
            <button className="icon-btn" onClick={() => invoke("run_power_action", { action: "sleep" })} title="Sleep">
              <Power size={14} />
            </button>
          </div>
          <div style={{ fontSize: 12, color: "#94a3b8", marginBottom: 6 }}>Nearby Wi-Fi</div>
          <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
            {wifiNets.slice(0, 6).map((w) => (
              <div key={w.ssid} className="dl-entry" style={{ justifyContent: "space-between" }}>
                <span>{w.ssid}{w.connected ? " (Connected)" : ""}</span>
                <span>{w.signalQuality}%</span>
              </div>
            ))}
          </div>
        </div>
      )}

      <div
        className={`dock-pill mat-${config.glassMaterial} theme-${config.theme}`}
        onMouseLeave={() => setHoveredIdx(null)}
      >
        {items.map((item, idx) => {
          const scale = calcScale(idx);
          return (
            <button
              key={item.key}
              className="dock-item"
              onMouseEnter={() => setHoveredIdx(idx)}
              onClick={() => {
                if (item.hwnd) {
                  invoke("toggle_window", { hwnd: item.hwnd });
                } else {
                  invoke("launch_app", { path: item.path, args: item.args });
                }
              }}
              style={{
                transform: `scale(${scale}) translateY(${-(scale - 1) * 18}px)`,
                marginLeft: scale > 1.05 ? 4 : 0,
                marginRight: scale > 1.05 ? 4 : 0,
              }}
            >
              <span className="dock-tooltip">{item.name}</span>
              <span
                className={`dock-icon-box style-${config.iconStyle}`}
                style={{ width: iconSize, height: iconSize }}
              >
                {item.icon ? (
                  <img className="dock-icon-img" src={item.icon} alt={item.name} />
                ) : (
                  <AppWindow size={22} />
                )}
              </span>
              <span
                className={`dock-dot ${item.hwnd ? "running" : ""} ${
                  item.isFocused ? "focused" : ""
                }`}
              />
            </button>
          );
        })}

        <div className="dock-divider" />

        <button className="dock-item" onClick={() => invoke("toggle_spotlight")}>
          <span className="dock-tooltip">Spotlight (Alt+Space)</span>
          <span className="dock-icon-box style-squircle" style={{ width: iconSize, height: iconSize }}>
            <Search size={19} />
          </span>
          <span className="dock-dot" />
        </button>

        <button className="dock-item" onClick={() => togglePopover("mixer")}>
          <span className="dock-tooltip">Volume Mixer</span>
          <span className="dock-icon-box style-squircle" style={{ width: iconSize, height: iconSize }}>
            <Volume2 size={19} />
          </span>
          <span className="dock-dot" />
        </button>

        <button className="dock-item" onClick={() => togglePopover("control")}>
          <span className="dock-tooltip">Control Center</span>
          <span className="dock-icon-box style-squircle" style={{ width: iconSize, height: iconSize }}>
            <SlidersHorizontal size={19} />
          </span>
          <span className="dock-dot" />
        </button>

        <button className="dock-item" onClick={() => togglePopover("downloads")}>
          <span className="dock-tooltip">{t.downloads}</span>
          <span className="dock-icon-box style-squircle" style={{ width: iconSize, height: iconSize }}>
            <FolderDown size={19} />
          </span>
          <span className="dock-dot" />
        </button>

        <button
          className="dock-item"
          onClick={() => invoke("open_trash")}
          onContextMenu={(e) => {
            e.preventDefault();
            invoke("empty_trash");
          }}
        >
          <span className="dock-tooltip">
            {t.trash} ({trash.items})
          </span>
          <span className="dock-icon-box style-squircle" style={{ width: iconSize, height: iconSize }}>
            <Trash2 size={19} />
          </span>
          <span className="dock-dot" />
        </button>

        <button className="dock-item" onClick={() => invoke("open_settings")}>
          <span className="dock-tooltip">{t.settings}</span>
          <span className="dock-icon-box style-squircle" style={{ width: iconSize, height: iconSize }}>
            <Settings size={19} />
          </span>
          <span className="dock-dot" />
        </button>
      </div>
    </div>
  );
}
