import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AppWindow,
  ArrowLeft,
  BatteryCharging,
  Bell,
  Bluetooth,
  Calendar,
  Check,
  ChevronLeft,
  ChevronRight,
  ChevronUp,
  ChevronsLeft,
  ChevronsRight,
  FolderDown,
  Headphones,
  Lock,
  Monitor,
  Power,
  Search,
  Settings,
  SlidersHorizontal,
  Trash2,
  Volume2,
  VolumeX,
  Wifi,
  X,
} from "lucide-react";
import {
  AppConfig,
  AudioOutputDevice,
  AudioSessionInfo,
  DownloadEntry,
  IcsEvent,
  NotificationsSnapshot,
  RunningWindow,
  SystemStatus,
  TrashInfo,
  WifiNetwork,
} from "../types";
import { STRINGS } from "../i18n";
import {
  buildMonthCells,
  computeDockDimensions,
  isValidHttpsUrl,
  toLocalDateKey,
} from "../utils";

type PopoverKind = "none" | "mixer" | "hidden" | "downloads" | "control" | "calendar";

const WEEK_DAYS = ["M", "T", "W", "T", "F", "S", "S"];

export function DockWindow() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [windows, setWindows] = useState<RunningWindow[]>([]);
  const [windowsLoaded, setWindowsLoaded] = useState(false);
  const [measuredPillWidth, setMeasuredPillWidth] = useState(0);
  const [hoveredIdx, setHoveredIdx] = useState<number | null>(null);
  const [previewData, setPreviewData] = useState<{ hwnd: number; img: string | null } | null>(null);
  const [fullscreenHidden, setFullscreenHidden] = useState(false);
  const [popover, setPopover] = useState<PopoverKind>("none");
  const [downloads, setDownloads] = useState<DownloadEntry[]>([]);
  const [audioSessions, setAudioSessions] = useState<AudioSessionInfo[]>([]);
  const [audioOutputs, setAudioOutputs] = useState<AudioOutputDevice[]>([]);
  const [wifiNets, setWifiNets] = useState<WifiNetwork[]>([]);
  const [icsEvents, setIcsEvents] = useState<IcsEvent[]>([]);
  const [actionError, setActionError] = useState<string | null>(null);
  const [notifications, setNotifications] = useState<NotificationsSnapshot>({ status: "unspecified", items: [] });
  const [sysStatus, setSysStatus] = useState<SystemStatus>({
    batteryPercent: 100,
    acOnline: true,
    hasBattery: false,
  });
  const [trash, setTrash] = useState<TrashInfo>({ items: 0, sizeBytes: 0 });
  const [nowDate, setNowDate] = useState(() => new Date());
  const [calMonth, setCalMonth] = useState(() => ({ year: new Date().getFullYear(), month: new Date().getMonth() }));
  const [selectedDateStr, setSelectedDateStr] = useState(() => toLocalDateKey(new Date()));

  const pillRef = useRef<HTMLDivElement | null>(null);
  const previewTimerRef = useRef<number | null>(null);
  const leaveTimerRef = useRef<number | null>(null);

  const reportError = (msg: unknown) => {
    const text = typeof msg === "string" ? msg : msg instanceof Error ? msg.message : "Action failed";
    setActionError(text);
    window.setTimeout(() => setActionError((prev) => (prev === text ? null : prev)), 2500);
  };

  useEffect(() => {
    invoke<AppConfig>("get_config").then(setConfig).catch(reportError);
    const refreshWindows = () => {
      if (document.hidden) return;
      invoke<RunningWindow[]>("get_running_windows")
        .then((list) => {
          setWindows(list);
          setWindowsLoaded(true);
        })
        .catch(() => setWindowsLoaded(true));
    };
    const refreshSlow = () => {
      if (document.hidden) return;
      invoke<TrashInfo>("get_trash_info").then(setTrash).catch(() => {});
      invoke<SystemStatus>("get_system_status").then(setSysStatus).catch(() => {});
    };
    refreshWindows();
    refreshSlow();
    const fastTimer = setInterval(refreshWindows, 2200);
    const slowTimer = setInterval(refreshSlow, 12000);
    const clockTimer = setInterval(() => setNowDate(new Date()), 10000);
    const unlistenCfg = listen<AppConfig>("antibar://config-changed", (e) => setConfig(e.payload));
    const unlistenFs = listen<boolean>("antibar://fullscreen-changed", (e) => setFullscreenHidden(e.payload));
    return () => {
      clearInterval(fastTimer);
      clearInterval(slowTimer);
      clearInterval(clockTimer);
      if (previewTimerRef.current) window.clearTimeout(previewTimerRef.current);
      if (leaveTimerRef.current) window.clearTimeout(leaveTimerRef.current);
      unlistenCfg.then((u) => u());
      unlistenFs.then((u) => u());
    };
  }, []);

  useEffect(() => {
    if (!config?.showDockClock && popover !== "calendar" && popover !== "hidden") return;
    const pollNotifs = () => {
      if (document.hidden) return;
      invoke<NotificationsSnapshot>("get_notifications", { requestAccess: false })
        .then(setNotifications)
        .catch(() => {});
    };
    pollNotifs();
    const id = setInterval(pollNotifs, 8000);
    return () => clearInterval(id);
  }, [config?.showDockClock, popover]);

  const items = useMemo(() => {
    if (!config) return [];
    return windows.slice(0, 12).map((w) => {
      const customPin = config.pinnedApps.find(
        (p) =>
          w.exeName.toLowerCase() === p.path.toLowerCase() ||
          w.exePath.toLowerCase() === p.path.toLowerCase()
      );
      return {
        key: `win:${w.hwnd}`,
        name: w.title,
        exeName: w.exeName,
        path: w.exePath,
        hwnd: w.hwnd,
        isFocused: w.isFocused,
        isMinimized: w.isMinimized,
        icon: w.iconDataUrl ?? customPin?.iconDataUrl ?? null,
      };
    });
  }, [config, windows]);

  const hoveredItem = hoveredIdx !== null ? items[hoveredIdx] ?? null : null;
  const iconSize = config?.iconSize || 48;
  const maxScale = config?.magnification || 1.38;

  useEffect(() => {
    const el = pillRef.current;
    if (!el) return;
    const measure = () => {
      const w = Math.ceil(el.getBoundingClientRect().width);
      if (w > 0) setMeasuredPillWidth(w);
    };
    measure();
    const ro = new ResizeObserver(measure);
    ro.observe(el);
    return () => ro.disconnect();
  }, [config, items.length, iconSize]);

  const { width: dockWidth, height: dockHeight } = useMemo(
    () =>
      computeDockDimensions({
        itemCount: items.length,
        iconSize,
        measuredPillWidth,
        showDockClock: Boolean(config?.showDockClock),
        popover,
        hasPreview: Boolean(hoveredItem),
        maxScreenWidth: window.screen?.availWidth || 1920,
      }),
    [items.length, iconSize, measuredPillWidth, config?.showDockClock, popover, hoveredItem]
  );

  useEffect(() => {
    if (!config || !windowsLoaded) return;
    invoke("resize_overlay", {
      label: "dock",
      width: dockWidth,
      height: dockHeight,
    }).catch(() => {});
  }, [config, windowsLoaded, dockWidth, dockHeight]);

  if (!config) return null;
  if (config.hideInFullscreen && fullscreenHidden) return null;

  const calcScale = (idx: number) => {
    if (hoveredIdx === null) return 1;
    const dist = Math.abs(idx - hoveredIdx);
    if (dist === 0) return maxScale;
    if (dist === 1) return 1 + (maxScale - 1) * 0.52;
    if (dist === 2) return 1 + (maxScale - 1) * 0.18;
    return 1;
  };

  const handleAppHover = (idx: number, hwnd: number) => {
    if (leaveTimerRef.current) {
      window.clearTimeout(leaveTimerRef.current);
      leaveTimerRef.current = null;
    }
    setHoveredIdx(idx);
    if (popover !== "none") return;
    if (previewTimerRef.current) window.clearTimeout(previewTimerRef.current);
    previewTimerRef.current = window.setTimeout(() => {
      invoke<string | null>("get_window_preview", { hwnd })
        .then((img) => setPreviewData({ hwnd, img }))
        .catch(() => setPreviewData({ hwnd, img: null }));
    }, 150);
  };

  const handleDockLeave = () => {
    if (previewTimerRef.current) {
      window.clearTimeout(previewTimerRef.current);
      previewTimerRef.current = null;
    }
    leaveTimerRef.current = window.setTimeout(() => {
      setHoveredIdx(null);
      setPreviewData(null);
    }, 130);
  };

  const handleDockEnter = () => {
    if (leaveTimerRef.current) {
      window.clearTimeout(leaveTimerRef.current);
      leaveTimerRef.current = null;
    }
  };

  const openSubPopover = async (next: PopoverKind) => {
    const resolved: PopoverKind = popover === next ? "none" : next;
    setPopover(resolved);
    setHoveredIdx(null);
    setPreviewData(null);
    try {
      if (resolved === "downloads") {
        setDownloads(await invoke<DownloadEntry[]>("get_downloads"));
      } else if (resolved === "mixer") {
        const [sess, outs] = await Promise.all([
          invoke<AudioSessionInfo[]>("get_audio_sessions"),
          invoke<AudioOutputDevice[]>("get_audio_outputs"),
        ]);
        setAudioSessions(sess);
        setAudioOutputs(outs);
      } else if (resolved === "control") {
        setWifiNets(await invoke<WifiNetwork[]>("get_wifi_networks"));
        setSysStatus(await invoke<SystemStatus>("get_system_status"));
      } else if (resolved === "calendar") {
        const snap = await invoke<NotificationsSnapshot>("get_notifications", { requestAccess: true }).catch(
          (): NotificationsSnapshot => ({ status: "unsupported", items: [] })
        );
        setNotifications(snap);
        if (config && isValidHttpsUrl(config.calendarIcsUrl)) {
          invoke<IcsEvent[]>("fetch_ics_agenda", { url: config.calendarIcsUrl })
            .then(setIcsEvents)
            .catch(() => {});
        } else {
          setIcsEvents([]);
        }
      }
    } catch (err) {
      reportError(err);
    }
  };

  const updateVolume = async (sess: AudioSessionInfo, vol: number, muted: boolean) => {
    setAudioSessions((prev) =>
      prev.map((s) => (s.pid === sess.pid && s.isMaster === sess.isMaster ? { ...s, volume: vol, muted } : s))
    );
    await invoke("set_app_volume", { pid: sess.pid, isMaster: sess.isMaster, volume: vol, muted }).catch(reportError);
  };

  const switchOutput = async (deviceId: string) => {
    await invoke("set_audio_output", { deviceId }).catch(reportError);
    const [sess, outs] = await Promise.all([
      invoke<AudioSessionInfo[]>("get_audio_sessions").catch(() => []),
      invoke<AudioOutputDevice[]>("get_audio_outputs").catch(() => []),
    ]);
    setAudioSessions(sess);
    setAudioOutputs(outs);
  };

  const calCells = buildMonthCells(calMonth.year, calMonth.month);
  const todayStr = toLocalDateKey(nowDate);
  const monthLabel = new Date(calMonth.year, calMonth.month, 1).toLocaleDateString(undefined, {
    month: "long",
    year: "numeric",
  });
  const dayAgenda = config.agendaItems.filter((a) => a.date === selectedDateStr);
  const compactKey = selectedDateStr.replace(/-/g, "");
  const dayIcsEvents = icsEvents.filter((e) => !e.dtstart || e.dtstart.startsWith(compactKey));
  const previewLeft =
    hoveredIdx !== null
      ? Math.max(8, Math.min(dockWidth - 252, Math.round(36 + hoveredIdx * (iconSize + 8) + iconSize / 2 - 122)))
      : 8;

  return (
    <div className={`dock-shell theme-${config.theme}`}>
      {actionError && (
        <div className="dock-tooltip" style={{ opacity: 1, bottom: 96, transform: "none" }}>
          {actionError}
        </div>
      )}
      {popover === "none" && hoveredItem && (
        <div
          className="dock-preview-card"
          style={{ left: previewLeft }}
          onMouseEnter={handleDockEnter}
          onMouseLeave={handleDockLeave}
        >
          <div className="dock-preview-head">
            <div style={{ display: "flex", alignItems: "center", gap: 7, minWidth: 0 }}>
              {hoveredItem.icon ? (
                <img src={hoveredItem.icon} alt="" style={{ width: 15, height: 15, objectFit: "contain" }} />
              ) : (
                <AppWindow size={14} />
              )}
              <span style={{ fontSize: 12, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                {hoveredItem.name}
              </span>
            </div>
            <button
              className="icon-btn"
              style={{ padding: 3 }}
              title="Close window"
              onClick={(e) => {
                e.stopPropagation();
                invoke("close_window", { hwnd: hoveredItem.hwnd }).catch(reportError);
                setHoveredIdx(null);
                setPreviewData(null);
              }}
            >
              <X size={12} />
            </button>
          </div>
          <button
            className="dock-preview-thumb-btn"
            onClick={() => invoke("toggle_window", { hwnd: hoveredItem.hwnd })}
          >
            {previewData && previewData.hwnd === hoveredItem.hwnd && previewData.img ? (
              <img src={previewData.img} alt={hoveredItem.name} className="dock-preview-img" />
            ) : (
              <div className="dock-preview-fallback">
                {hoveredItem.icon ? (
                  <img src={hoveredItem.icon} alt="" style={{ width: 32, height: 32, objectFit: "contain" }} />
                ) : (
                  <AppWindow size={28} />
                )}
                <span className="muted-label" style={{ fontSize: 11 }}>
                  {hoveredItem.isMinimized ? "Minimized · Click to restore" : hoveredItem.exeName}
                </span>
              </div>
            )}
          </button>
        </div>
      )}

      {popover === "mixer" && (
        <div className="dock-popover" style={{ width: 324 }}>
          <div className="dock-popover-head">
            <span>{STRINGS.soundMixer}</span>
            <button
              className="icon-btn"
              onClick={() => invoke("launch_app", { path: "ms-settings:sound", args: null })}
            >
              Sound Settings
            </button>
          </div>
          <div style={{ display: "flex", flexDirection: "column", gap: 9 }}>
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
                <span style={{ fontSize: 11, width: 32, textAlign: "right", fontVariantNumeric: "tabular-nums" }}>
                  {Math.round(s.volume * 100)}%
                </span>
              </div>
            ))}
          </div>

          {audioOutputs.length > 0 && (
            <>
              <div className="muted-label" style={{ fontSize: 11, fontWeight: 600, marginTop: 12, marginBottom: 6 }}>
                Output
              </div>
              <div style={{ display: "flex", flexDirection: "column", gap: 4 }}>
                {audioOutputs.map((dev) => (
                  <button
                    key={dev.id}
                    className={`dl-entry ${dev.isDefault ? "is-active-row" : ""}`}
                    style={{ justifyContent: "space-between" }}
                    onClick={() => switchOutput(dev.id)}
                  >
                    <span style={{ display: "inline-flex", alignItems: "center", gap: 8, minWidth: 0 }}>
                      <Headphones size={13} />
                      <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {dev.name}
                      </span>
                    </span>
                    {dev.isDefault && <Check size={13} color="#0ea5e9" />}
                  </button>
                ))}
              </div>
            </>
          )}
        </div>
      )}

      {popover === "hidden" && (
        <div className="dock-popover">
          <div className="dock-popover-head">
            <span>{STRINGS.hiddenIcons}</span>
            <button
              className="icon-btn"
              onClick={() => {
                invoke("open_hidden_tray").catch(() => {});
                setPopover("none");
              }}
              title="Open Windows system tray overflow (Win+B)"
            >
              Windows Tray
            </button>
          </div>
          <div className="hidden-grid">
            <button
              className="dl-entry"
              onClick={() => {
                invoke("toggle_spotlight");
                setPopover("none");
              }}
            >
              <Search size={14} />
              <span>Spotlight</span>
            </button>
            <button className="dl-entry" onClick={() => openSubPopover("calendar")}>
              <Calendar size={14} />
              <span>
                Calendar {notifications.items.length > 0 ? `(${notifications.items.length})` : ""}
              </span>
            </button>
            <button className="dl-entry" onClick={() => openSubPopover("control")}>
              <SlidersHorizontal size={14} />
              <span>Control Center</span>
            </button>
            <button className="dl-entry" onClick={() => openSubPopover("downloads")}>
              <FolderDown size={14} />
              <span>{STRINGS.downloads}</span>
            </button>
            <button
              className="dl-entry"
              title="Left click to open, right click to empty"
              onClick={() => invoke("open_trash").catch(reportError)}
              onContextMenu={(e) => {
                e.preventDefault();
                invoke("empty_trash").catch(reportError);
              }}
            >
              <Trash2 size={14} />
              <span>
                {STRINGS.trash} ({trash.items})
              </span>
            </button>
            <button
              className="dl-entry"
              onClick={() =>
                invoke("update_config", {
                  newConfig: { ...config, showDockClock: !config.showDockClock },
                })
              }
            >
              <Bell size={14} />
              <span>{config.showDockClock ? "Hide Clock Pill" : "Show Clock Pill"}</span>
            </button>
          </div>
          <button
            className="dl-entry"
            style={{ width: "100%", marginTop: 8, justifyContent: "space-between" }}
            onClick={() =>
              invoke("update_config", {
                newConfig: { ...config, hideWindowsTaskbar: !config.hideWindowsTaskbar },
              })
            }
          >
            <span style={{ display: "inline-flex", alignItems: "center", gap: 8 }}>
              <Monitor size={14} /> Windows Taskbar
            </span>
            <span className="muted-label">{config.hideWindowsTaskbar ? "Hidden" : "Visible"}</span>
          </button>
        </div>
      )}

      {popover === "calendar" && (
        <div className="dock-popover calendar-notif-popover">
          <div className="cal-pane">
            <div style={{ fontSize: 12, fontWeight: 600, marginBottom: 8 }}>
              {nowDate.toLocaleDateString(undefined, { weekday: "long", month: "long", day: "numeric", year: "numeric" })}
            </div>
            <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 2 }}>
                <button
                  className="icon-btn"
                  style={{ padding: 3 }}
                  onClick={() => setCalMonth((m) => ({ ...m, year: m.year - 1 }))}
                >
                  <ChevronsLeft size={12} />
                </button>
                <button
                  className="icon-btn"
                  style={{ padding: 3 }}
                  onClick={() =>
                    setCalMonth((m) => (m.month === 0 ? { year: m.year - 1, month: 11 } : { ...m, month: m.month - 1 }))
                  }
                >
                  <ChevronLeft size={12} />
                </button>
              </div>
              <span style={{ fontSize: 12, fontWeight: 600 }}>{monthLabel}</span>
              <div style={{ display: "flex", alignItems: "center", gap: 2 }}>
                <button
                  className="icon-btn"
                  style={{ padding: 3 }}
                  onClick={() =>
                    setCalMonth((m) => (m.month === 11 ? { year: m.year + 1, month: 0 } : { ...m, month: m.month + 1 }))
                  }
                >
                  <ChevronRight size={12} />
                </button>
                <button
                  className="icon-btn"
                  style={{ padding: 3 }}
                  onClick={() => setCalMonth({ year: nowDate.getFullYear(), month: nowDate.getMonth() })}
                >
                  <ChevronsRight size={12} />
                </button>
              </div>
            </div>
            <div className="cal-grid">
              {WEEK_DAYS.map((d, i) => (
                <span key={i} className="cal-dow">{d}</span>
              ))}
              {calCells.map((cell) => {
                const isToday = cell.dateStr === todayStr;
                const isSel = cell.dateStr === selectedDateStr;
                return (
                  <button
                    key={cell.key}
                    className={`cal-day ${cell.inMonth ? "" : "dim"} ${isToday ? "today" : ""} ${isSel ? "selected" : ""}`}
                    onClick={() => setSelectedDateStr(cell.dateStr)}
                  >
                    {cell.day}
                  </button>
                );
              })}
            </div>
            <div style={{ marginTop: 10, paddingTop: 8, borderTop: "1px solid var(--line-subtle)", fontSize: 11 }}>
              <div style={{ fontWeight: 600, marginBottom: 4 }}>
                Agenda ({dayAgenda.length + dayIcsEvents.length})
              </div>
              {dayAgenda.length === 0 && dayIcsEvents.length === 0 ? (
                <div className="muted-label">No items for {selectedDateStr}</div>
              ) : (
                <>
                  {dayAgenda.slice(0, 2).map((a) => (
                    <div key={a.id} className="muted-label" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {a.time} · {a.title}
                    </div>
                  ))}
                  {dayIcsEvents.slice(0, 2).map((ev, i) => (
                    <div key={`${ev.dtstart}-${i}`} className="muted-label" style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                      {ev.summary}
                    </div>
                  ))}
                </>
              )}
            </div>
          </div>

          <div className="notif-pane">
            <div className="dock-popover-head" style={{ marginBottom: 6 }}>
              <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
                <button className="icon-btn" onClick={() => setPopover("hidden")} style={{ padding: "3px 6px" }}>
                  <ArrowLeft size={12} />
                </button>
                Notifications ({notifications.items.length})
              </span>
              {notifications.items.length > 0 && (
                <button
                  className="icon-btn"
                  onClick={() => {
                    invoke("clear_notifications").then(() =>
                      setNotifications((prev) => ({ ...prev, items: [] }))
                    );
                  }}
                >
                  Clear All
                </button>
              )}
            </div>
            {notifications.status !== "allowed" && (
              <div className="dl-entry" style={{ justifyContent: "space-between", marginBottom: 8 }}>
                <span>Toast access: {notifications.status}</span>
                <button
                  className="icon-btn"
                  onClick={() =>
                    invoke<NotificationsSnapshot>("get_notifications", { requestAccess: true }).then(setNotifications)
                  }
                >
                  Allow
                </button>
              </div>
            )}
            <div style={{ display: "flex", flexDirection: "column", gap: 6, maxHeight: 224, overflowY: "auto" }}>
              {notifications.items.length === 0 ? (
                <div className="muted-label" style={{ padding: "28px 8px", textAlign: "center", fontSize: 12 }}>
                  No notifications
                </div>
              ) : (
                notifications.items.map((n) => (
                  <div key={n.id} className="notif-card">
                    <div style={{ flex: 1, minWidth: 0 }}>
                      <div style={{ display: "flex", justifyContent: "space-between", gap: 6, fontSize: 10 }} className="muted-label">
                        <span>{n.appName}</span>
                        <span>{n.timestampMs > 0 ? new Date(n.timestampMs).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" }) : ""}</span>
                      </div>
                      <div style={{ fontSize: 12, fontWeight: 600, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                        {n.title}
                      </div>
                      {n.body && (
                        <div className="muted-label" style={{ fontSize: 11, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                          {n.body}
                        </div>
                      )}
                    </div>
                    <button
                      className="icon-btn"
                      style={{ padding: 3, alignSelf: "flex-start" }}
                      onClick={() => {
                        invoke("dismiss_notification", { id: n.id }).then(() =>
                          setNotifications((prev) => ({
                            ...prev,
                            items: prev.items.filter((x) => x.id !== n.id),
                          }))
                        );
                      }}
                    >
                      <X size={11} />
                    </button>
                  </div>
                ))
              )}
            </div>
          </div>
        </div>
      )}

      {popover === "downloads" && (
        <div className="dock-popover">
          <div className="dock-popover-head">
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <button className="icon-btn" onClick={() => setPopover("hidden")} style={{ padding: "3px 6px" }}>
                <ArrowLeft size={12} />
              </button>
              {STRINGS.downloads}
            </span>
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

      {popover === "control" && (
        <div className="dock-popover">
          <div className="dock-popover-head">
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6 }}>
              <button className="icon-btn" onClick={() => setPopover("hidden")} style={{ padding: "3px 6px" }}>
                <ArrowLeft size={12} />
              </button>
              Control Center
            </span>
            {sysStatus.hasBattery && (
              <span style={{ fontSize: 12, color: "#0ea5e9", display: "inline-flex", alignItems: "center", gap: 4 }}>
                <BatteryCharging size={14} /> {sysStatus.batteryPercent}%
              </span>
            )}
          </div>
          <div style={{ display: "flex", gap: 6, marginBottom: 10 }}>
            <button className="icon-btn" style={{ flex: 1, gap: 6 }} onClick={() => invoke("run_power_action", { action: "wifi" }).catch(reportError)}>
              <Wifi size={14} /> Wi-Fi
            </button>
            <button className="icon-btn" style={{ flex: 1, gap: 6 }} onClick={() => invoke("run_power_action", { action: "bluetooth" }).catch(reportError)}>
              <Bluetooth size={14} /> Bluetooth
            </button>
            <button className="icon-btn" onClick={() => invoke("run_power_action", { action: "lock" }).catch(reportError)} title="Lock PC">
              <Lock size={14} />
            </button>
            <button className="icon-btn" onClick={() => invoke("run_power_action", { action: "sleep" }).catch(reportError)} title="Sleep">
              <Power size={14} />
            </button>
          </div>
          <div className="muted-label" style={{ fontSize: 12, marginBottom: 6 }}>
            Nearby Wi-Fi
          </div>
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
        ref={pillRef}
        className={`dock-pill mat-${config.glassMaterial} theme-${config.theme}`}
        onMouseEnter={handleDockEnter}
        onMouseLeave={handleDockLeave}
      >
        {items.length === 0 ? (
          <span className="muted-label" style={{ fontSize: 12, padding: "0 10px" }}>
            No open apps
          </span>
        ) : (
          items.map((item, idx) => {
            const scale = calcScale(idx);
            return (
              <button
                key={item.key}
                className="dock-item"
                onMouseEnter={() => handleAppHover(idx, item.hwnd)}
                onClick={() => invoke("toggle_window", { hwnd: item.hwnd })}
                style={{
                  transform: `scale(${scale}) translateY(${-(scale - 1) * 18}px)`,
                  marginLeft: scale > 1.05 ? 4 : 0,
                  marginRight: scale > 1.05 ? 4 : 0,
                }}
              >
                <span
                  className={`dock-icon-box style-${config.iconStyle} clean-app-tile`}
                  style={{ width: iconSize, height: iconSize }}
                >
                  {item.icon ? (
                    <img className="dock-icon-img" src={item.icon} alt={item.name} />
                  ) : (
                    <AppWindow size={22} />
                  )}
                </span>
                <span
                  className={`dock-dot running ${item.isFocused ? "focused" : ""}`}
                />
              </button>
            );
          })
        )}

        <div className="dock-divider" />

        <button className="dock-item" onClick={() => openSubPopover("mixer")}>
          <span className="dock-tooltip">{STRINGS.soundMixer}</span>
          <span className="dock-icon-box utility-tile" style={{ width: iconSize - 6, height: iconSize - 6 }}>
            <Volume2 size={18} />
          </span>
          <span className={`dock-dot ${popover === "mixer" ? "focused" : ""}`} />
        </button>

        <button className="dock-item" onClick={() => openSubPopover("hidden")}>
          <span className="dock-tooltip">{STRINGS.hiddenIcons}</span>
          <span className="dock-icon-box utility-tile" style={{ width: iconSize - 6, height: iconSize - 6 }}>
            <ChevronUp size={18} />
          </span>
          <span
            className={`dock-dot ${
              popover === "hidden" || popover === "downloads" || popover === "control" || popover === "calendar"
                ? "focused"
                : ""
            }`}
          />
        </button>

        <button className="dock-item" onClick={() => invoke("open_settings").catch(reportError)}>
          <span className="dock-tooltip">{STRINGS.settings}</span>
          <span className="dock-icon-box utility-tile" style={{ width: iconSize - 6, height: iconSize - 6 }}>
            <Settings size={18} />
          </span>
          <span className="dock-dot" />
        </button>

        {config.showDockClock && (
          <button
            className="dock-clock-btn"
            onClick={() => openSubPopover("calendar")}
            title="Calendar & Notifications"
          >
            <div style={{ display: "flex", flexDirection: "column", alignItems: "flex-end", lineHeight: 1.15 }}>
              <span style={{ fontSize: 11, fontWeight: 700, fontVariantNumeric: "tabular-nums" }}>
                {nowDate.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })}
              </span>
              <span className="muted-label" style={{ fontSize: 10, fontVariantNumeric: "tabular-nums" }}>
                {nowDate.toLocaleDateString([], { month: "2-digit", day: "2-digit", year: "numeric" })}
              </span>
            </div>
            <span style={{ position: "relative", display: "inline-flex" }}>
              <Bell size={14} />
              {notifications.items.length > 0 && (
                <span className="dock-bell-badge">{notifications.items.length}</span>
              )}
            </span>
          </button>
        )}
      </div>
    </div>
  );
}
