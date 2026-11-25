import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Layers, LayoutGrid, Palette, Sliders, Sparkles } from "lucide-react";
import { AppConfig, IcsEvent } from "../types";
import { STRINGS } from "../i18n";
import { isValidHttpsUrl } from "../utils";

type Tab = "appearance" | "dock" | "notch" | "widgets" | "behavior";

export function SettingsWindow() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [tab, setTab] = useState<Tab>("appearance");
  const [feedStatus, setFeedStatus] = useState<string>("");
  const saveTimer = useRef<number | null>(null);
  const pendingConfig = useRef<AppConfig | null>(null);

  useEffect(() => {
    invoke<AppConfig>("get_config").then(setConfig).catch(() => {});
    const unlistenCfg = listen<AppConfig>("antibar://config-changed", (e) => setConfig(e.payload));
    return () => {
      if (saveTimer.current) {
        window.clearTimeout(saveTimer.current);
        if (pendingConfig.current) {
          invoke("update_config", { newConfig: pendingConfig.current }).catch(() => {});
        }
      }
      unlistenCfg.then((u) => u());
    };
  }, []);

  const patch = (partial: Partial<AppConfig>) => {
    if (!config) return;
    const next = { ...config, ...partial };
    setConfig(next);
    pendingConfig.current = next;
    if (saveTimer.current) window.clearTimeout(saveTimer.current);
    saveTimer.current = window.setTimeout(() => {
      const target = pendingConfig.current;
      pendingConfig.current = null;
      if (target) {
        invoke("update_config", { newConfig: target }).catch(console.error);
      }
    }, 70);
  };

  if (!config) return null;

  return (
    <div className={`settings-shell theme-${config.theme}`}>
      <aside className="settings-sidebar">
        <div style={{ fontSize: 16, fontWeight: 700, padding: "0 12px 14px" }}>Anti-Bar</div>
        <button className={`settings-nav-btn ${tab === "appearance" ? "active" : ""}`} onClick={() => setTab("appearance")}>
          <Palette size={16} /> {STRINGS.tabAppearance}
        </button>
        <button className={`settings-nav-btn ${tab === "dock" ? "active" : ""}`} onClick={() => setTab("dock")}>
          <Layers size={16} /> {STRINGS.tabDock}
        </button>
        <button className={`settings-nav-btn ${tab === "notch" ? "active" : ""}`} onClick={() => setTab("notch")}>
          <Sparkles size={16} /> {STRINGS.tabNotch}
        </button>
        <button className={`settings-nav-btn ${tab === "widgets" ? "active" : ""}`} onClick={() => setTab("widgets")}>
          <LayoutGrid size={16} /> {STRINGS.tabWidgets}
        </button>
        <button className={`settings-nav-btn ${tab === "behavior" ? "active" : ""}`} onClick={() => setTab("behavior")}>
          <Sliders size={16} /> {STRINGS.tabBehavior}
        </button>
      </aside>

      <main className="settings-main">
        <h2 style={{ marginTop: 0 }}>{STRINGS.settingsTitle}</h2>

        {tab === "appearance" && (
          <div className="setting-group">
            <div className="setting-row">
              <span>{STRINGS.themeLabel}</span>
              <select className="pill-select" value={config.theme} onChange={(e) => patch({ theme: e.target.value })}>
                <option value="dark">Dark</option>
                <option value="light">Light</option>
              </select>
            </div>
            <div className="setting-row">
              <span>{STRINGS.materialLabel}</span>
              <select className="pill-select" value={config.glassMaterial} onChange={(e) => patch({ glassMaterial: e.target.value })}>
                <option value="frosted">Frosted</option>
                <option value="liquid">Liquid</option>
                <option value="matte">Matte</option>
                <option value="classic">Classic</option>
              </select>
            </div>
            <div className="setting-row">
              <span>{STRINGS.iconStyle}</span>
              <select className="pill-select" value={config.iconStyle} onChange={(e) => patch({ iconStyle: e.target.value })}>
                <option value="squircle">Squircle</option>
                <option value="windows">Windows</option>
                <option value="normalized">Normalized</option>
              </select>
            </div>
          </div>
        )}

        {tab === "dock" && (
          <div className="setting-group">
            <div className="setting-row">
              <span>{STRINGS.iconSize}: {config.iconSize}px</span>
              <input type="range" min={36} max={64} value={config.iconSize} onChange={(e) => patch({ iconSize: Number(e.target.value) })} />
            </div>
            <div className="setting-row">
              <span>{STRINGS.magnification}: {config.magnification.toFixed(2)}x</span>
              <input type="range" min={1.0} max={1.65} step={0.05} value={config.magnification} onChange={(e) => patch({ magnification: Number(e.target.value) })} />
            </div>
            <div className="setting-row">
              <span>{STRINGS.hideInFullscreen}</span>
              <input type="checkbox" checked={config.hideInFullscreen} onChange={(e) => patch({ hideInFullscreen: e.target.checked })} />
            </div>
            <div className="setting-row">
              <span>Show Clock &amp; Notifications Pill on Dock</span>
              <input type="checkbox" checked={config.showDockClock} onChange={(e) => patch({ showDockClock: e.target.checked })} />
            </div>
          </div>
        )}

        {tab === "notch" && (
          <div className="setting-group">
            <div className="setting-row">
              <span>{STRINGS.notchEnabled}</span>
              <input type="checkbox" checked={config.notchEnabled} onChange={(e) => patch({ notchEnabled: e.target.checked })} />
            </div>
            <div className="setting-row">
              <span>{STRINGS.showLyrics}</span>
              <input type="checkbox" checked={config.showLyrics} onChange={(e) => patch({ showLyrics: e.target.checked })} />
            </div>
          </div>
        )}

        {tab === "widgets" && (
          <div className="setting-group">
            <div style={{ display: "grid", gridTemplateColumns: "1fr 1fr", gap: 12, marginBottom: 16 }}>
              <div className="setting-card">
                <div>
                  <div style={{ fontWeight: 600 }}>Desktop Clock</div>
                  <div className="muted-label" style={{ fontSize: 12 }}>Direct2D · {config.clockStyle}</div>
                </div>
                <input type="checkbox" checked={config.clockEnabled} onChange={(e) => patch({ clockEnabled: e.target.checked })} />
              </div>
              <div className="setting-card">
                <div>
                  <div style={{ fontWeight: 600 }}>Vinyl Player Preset</div>
                  <div className="muted-label" style={{ fontSize: 12 }}>SMTC in Notch · {config.vinylStyle}</div>
                </div>
                <input type="checkbox" checked={config.vinylEnabled} onChange={(e) => patch({ vinylEnabled: e.target.checked })} />
              </div>
              <div className="setting-card">
                <div>
                  <div style={{ fontWeight: 600 }}>Calendar &amp; Agenda</div>
                  <div className="muted-label" style={{ fontSize: 12 }}>Dock Calendar · .ics feed</div>
                </div>
                <input type="checkbox" checked={config.calendarEnabled} onChange={(e) => patch({ calendarEnabled: e.target.checked })} />
              </div>
              <div className="setting-card">
                <div>
                  <div style={{ fontWeight: 600 }}>Sticky Memo Preset</div>
                  <div className="muted-label" style={{ fontSize: 12 }}>Layout preset · {config.memoStyle}</div>
                </div>
                <input type="checkbox" checked={config.memoEnabled} onChange={(e) => patch({ memoEnabled: e.target.checked })} />
              </div>
            </div>

            <div className="setting-row">
              <span>Clock Style</span>
              <select className="pill-select" value={config.clockStyle} onChange={(e) => patch({ clockStyle: e.target.value })}>
                <option value="liquid">Liquid Glass</option>
                <option value="glass">Glass Card</option>
                <option value="sonoma">Sonoma</option>
              </select>
            </div>
            <div className="setting-row">
              <span>Clock Alignment</span>
              <select className="pill-select" value={config.clockAlign} onChange={(e) => patch({ clockAlign: e.target.value })}>
                <option value="left">Left</option>
                <option value="center">Center</option>
                <option value="right">Right</option>
              </select>
            </div>
            <div className="setting-row">
              <span>Clock Size: {config.clockSize}px</span>
              <input type="range" min={56} max={180} value={config.clockSize} onChange={(e) => patch({ clockSize: Number(e.target.value) })} />
            </div>
            <div className="setting-row">
              <div>
                <div>iCal (.ics) Feed URL</div>
                {feedStatus && <div className="muted-label" style={{ fontSize: 11 }}>{feedStatus}</div>}
              </div>
              <div style={{ display: "flex", gap: 6, alignItems: "center" }}>
                <input
                  type="text"
                  className="pill-select"
                  style={{ width: 220 }}
                  placeholder="https://calendar.google.com/..."
                  value={config.calendarIcsUrl}
                  onChange={(e) => {
                    setFeedStatus("");
                    patch({ calendarIcsUrl: e.target.value });
                  }}
                />
                <button
                  className="icon-btn"
                  onClick={() => {
                    if (!isValidHttpsUrl(config.calendarIcsUrl)) {
                      setFeedStatus("Enter a valid https:// URL");
                      return;
                    }
                    setFeedStatus("Checking...");
                    invoke<IcsEvent[]>("fetch_ics_agenda", { url: config.calendarIcsUrl })
                      .then((ev) => setFeedStatus(`Loaded ${ev.length} events`))
                      .catch(() => setFeedStatus("Failed to load feed"));
                  }}
                >
                  Test
                </button>
              </div>
            </div>
          </div>
        )}

        {tab === "behavior" && (
          <div className="setting-group">
            <div className="setting-row">
              <span>{STRINGS.genieEffect}</span>
              <input type="checkbox" checked={config.genieEffect} onChange={(e) => patch({ genieEffect: e.target.checked })} />
            </div>
            <div className="setting-row">
              <span>Use Windows Key for Spotlight (Ctrl+Esc fallback)</span>
              <input
                type="checkbox"
                checked={config.useWinKeyForLaunchpad}
                onChange={(e) => patch({ useWinKeyForLaunchpad: e.target.checked })}
              />
            </div>
            <div className="setting-row">
              <span>Hide Windows Taskbar (Opt-in)</span>
              <input
                type="checkbox"
                checked={config.hideWindowsTaskbar}
                onChange={(e) => patch({ hideWindowsTaskbar: e.target.checked })}
              />
            </div>
          </div>
        )}
      </main>
    </div>
  );
}
