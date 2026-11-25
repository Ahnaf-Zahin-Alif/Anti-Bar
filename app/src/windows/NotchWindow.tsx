import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Check,
  Clipboard,
  FileText,
  FolderOpen,
  Music,
  Pause,
  Pin,
  Play,
  RotateCcw,
  Search,
  Settings,
  SkipBack,
  SkipForward,
  Timer,
  Trash2,
} from "lucide-react";
import { AppConfig, ClipboardItem, LyricsResult, MediaState } from "../types";
import { STRINGS } from "../i18n";

type NotchTab = "media" | "shelf" | "timer" | "clipboard";
type ClipFilter = "all" | "text" | "image";

interface ShelfFile {
  name: string;
  size: number;
}

const TIMER_PRESETS = [5, 15, 25, 45, 60];

export function NotchWindow() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [tab, setTab] = useState<NotchTab>("media");
  const [media, setMedia] = useState<MediaState>({
    title: "",
    artist: "",
    album: "",
    isPlaying: false,
    positionSec: 0,
    durationSec: 0,
    hasSession: false,
    thumbnailDataUrl: null,
  });
  const [lyrics, setLyrics] = useState<LyricsResult | null>(null);
  const [clips, setClips] = useState<ClipboardItem[]>([]);
  const [clipQuery, setClipQuery] = useState("");
  const [clipFilter, setClipFilter] = useState<ClipFilter>("all");
  const [copiedId, setCopiedId] = useState<string | null>(null);
  const [shelf, setShelf] = useState<ShelfFile[]>([]);
  const [timerMinutes, setTimerMinutes] = useState(25);
  const [timerSec, setTimerSec] = useState(25 * 60);
  const [timerRunning, setTimerRunning] = useState(false);
  const [nowStr, setNowStr] = useState(() =>
    new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false })
  );
  const lastTrackRef = useRef("");
  const hoverTimerRef = useRef<number | null>(null);

  useEffect(() => {
    invoke<AppConfig>("get_config").then(setConfig).catch(() => {});
    invoke<ClipboardItem[]>("get_clipboard_history").then(setClips).catch(() => {});
    invoke<MediaState>("get_media_state").then(setMedia).catch(() => {});

    const unlistenMedia = listen<MediaState>("antibar://media-updated", (e) => {
      const next = e.payload;
      setMedia((prev) => ({
        ...next,
        thumbnailDataUrl:
          next.thumbnailDataUrl !== undefined && next.thumbnailDataUrl !== null
            ? next.thumbnailDataUrl
            : prev.title === next.title && prev.artist === next.artist
            ? prev.thumbnailDataUrl
            : null,
      }));
    });
    const unlistenClip = listen<ClipboardItem[]>("antibar://clipboard-updated", (e) => {
      setClips(e.payload);
    });
    const unlistenCfg = listen<AppConfig>("antibar://config-changed", (e) => setConfig(e.payload));
    const clockTick = setInterval(() => {
      setNowStr(new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", hour12: false }));
    }, 10000);

    return () => {
      clearInterval(clockTick);
      unlistenMedia.then((u) => u());
      unlistenClip.then((u) => u());
      unlistenCfg.then((u) => u());
    };
  }, []);

  useEffect(() => {
    if (!config?.showLyrics || !media.title) {
      setLyrics(null);
      return;
    }
    const key = `${media.title}::${media.artist}`;
    if (key === lastTrackRef.current) return;
    lastTrackRef.current = key;
    invoke<LyricsResult>("fetch_lyrics", {
      title: media.title,
      artist: media.artist,
      album: media.album,
      durationSec: media.durationSec,
    })
      .then(setLyrics)
      .catch(() => {});
  }, [media.title, media.artist, media.album, media.durationSec, config?.showLyrics]);

  useEffect(() => {
    if (!timerRunning) return;
    const id = setInterval(() => {
      setTimerSec((s) => {
        if (s <= 1) {
          setTimerRunning(false);
          return 0;
        }
        return s - 1;
      });
    }, 1000);
    return () => clearInterval(id);
  }, [timerRunning]);

  useEffect(() => {
    invoke("resize_overlay", {
      label: "notch",
      width: expanded ? 470 : 256,
      height: expanded ? 344 : 46,
    }).catch(() => {});
  }, [expanded]);

  const activeLyric =
    [...(lyrics?.synced ?? [])].reverse().find((l) => l.timeSec <= media.positionSec)?.text ?? "";

  const fmtTime = (sec: number) => {
    const safe = Math.max(0, Math.floor(sec));
    const m = Math.floor(safe / 60);
    const s = safe % 60;
    return `${m}:${String(s).padStart(2, "0")}`;
  };

  const filteredClips = useMemo(() => {
    const q = clipQuery.trim().toLowerCase();
    return clips.filter((c) => {
      if (clipFilter === "image" && c.kind !== "image") return false;
      if (clipFilter === "text" && c.kind === "image") return false;
      if (!q) return true;
      return c.content.toLowerCase().includes(q);
    });
  }, [clips, clipFilter, clipQuery]);

  const totalTimerSec = timerMinutes * 60;
  const timerProgress = totalTimerSec > 0 ? Math.max(0, Math.min(1, timerSec / totalTimerSec)) : 0;
  const ringRadius = 34;
  const ringCircumference = 2 * Math.PI * ringRadius;
  const ringOffset = ringCircumference * (1 - timerProgress);

  const handleMouseEnter = () => {
    if (hoverTimerRef.current) window.clearTimeout(hoverTimerRef.current);
    hoverTimerRef.current = window.setTimeout(() => {
      setExpanded(true);
    }, 130);
  };

  const handleMouseLeave = () => {
    if (hoverTimerRef.current) {
      window.clearTimeout(hoverTimerRef.current);
      hoverTimerRef.current = null;
    }
    if (!pinned) {
      setExpanded(false);
    }
  };

  return (
    <div className="notch-shell theme-dark">
      <div
        className={`notch-pill ${expanded ? "expanded" : ""}`}
        onMouseEnter={handleMouseEnter}
        onMouseLeave={handleMouseLeave}
      >
        {!expanded ? (
          <div className="notch-compact-row" onClick={() => setExpanded(true)}>
            <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
              {media.hasSession && media.thumbnailDataUrl ? (
                <img
                  src={media.thumbnailDataUrl}
                  alt=""
                  style={{ width: 20, height: 20, borderRadius: 6, objectFit: "cover", flexShrink: 0 }}
                />
              ) : (
                <span className={`notch-status-dot ${media.isPlaying ? "playing" : ""}`} />
              )}
            </div>
            <span style={{ fontWeight: 700, fontSize: 12, fontVariantNumeric: "tabular-nums", letterSpacing: "0.02em" }}>
              {timerRunning ? fmtTime(timerSec) : nowStr}
            </span>
            <div className={`notch-eq ${media.isPlaying ? "live" : ""}`} aria-hidden="true">
              <span />
              <span />
              <span />
              <span />
            </div>
          </div>
        ) : (
          <>
            <div className="notch-header">
              <div className="notch-tabs">
                <button className={`notch-tab ${tab === "media" ? "active" : ""}`} onClick={() => setTab("media")}>
                  <Music size={12} /> {STRINGS.mediaTab}
                </button>
                <button className={`notch-tab ${tab === "shelf" ? "active" : ""}`} onClick={() => setTab("shelf")}>
                  <FolderOpen size={12} /> {STRINGS.shelfTab}
                </button>
                <button className={`notch-tab ${tab === "timer" ? "active" : ""}`} onClick={() => setTab("timer")}>
                  <Timer size={12} /> {STRINGS.timerTab}
                </button>
                <button
                  className={`notch-tab ${tab === "clipboard" ? "active" : ""}`}
                  onClick={() => setTab("clipboard")}
                >
                  <Clipboard size={12} /> Clip
                </button>
              </div>
              <div style={{ display: "flex", alignItems: "center", gap: 4 }}>
                <button
                  className="icon-btn"
                  style={{ padding: 5 }}
                  onClick={() => invoke("open_settings")}
                  title="Settings"
                >
                  <Settings size={13} />
                </button>
                <button
                  className={`icon-btn ${pinned ? "is-pinned" : ""}`}
                  style={{ padding: 5, color: pinned ? "#38bdf8" : undefined }}
                  onClick={() => setPinned((p) => !p)}
                  title={pinned ? "Unpin Notch" : "Keep Notch Open"}
                >
                  <Pin size={13} />
                </button>
                <span style={{ fontSize: 12, fontWeight: 600, color: "#cbd5e1", marginLeft: 4, fontVariantNumeric: "tabular-nums" }}>
                  {nowStr}
                </span>
              </div>
            </div>

            <div className="notch-body">
              {tab === "media" && (
                <div>
                  {media.hasSession && media.title ? (
                    <>
                      <div style={{ display: "flex", alignItems: "center", gap: 12 }}>
                        {media.thumbnailDataUrl ? (
                          <img
                            src={media.thumbnailDataUrl}
                            alt=""
                            style={{ width: 56, height: 56, borderRadius: 12, objectFit: "cover", flexShrink: 0 }}
                          />
                        ) : (
                          <div
                            style={{
                              width: 56,
                              height: 56,
                              borderRadius: 12,
                              background: "rgba(56, 189, 248, 0.14)",
                              display: "flex",
                              alignItems: "center",
                              justifyContent: "center",
                              flexShrink: 0,
                            }}
                          >
                            <Music size={24} color="#38bdf8" />
                          </div>
                        )}
                        <div style={{ flex: 1, minWidth: 0 }}>
                          <div style={{ fontWeight: 700, fontSize: 14, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                            {media.title}
                          </div>
                          <div style={{ fontSize: 12, color: "#94a3b8", overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                            {[media.artist, media.album].filter(Boolean).join(" · ") || "Active session"}
                          </div>
                          {activeLyric && (
                            <div style={{ fontSize: 12, color: "#38bdf8", marginTop: 4, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                              {activeLyric}
                            </div>
                          )}
                        </div>
                        <div className={`notch-eq ${media.isPlaying ? "live" : ""}`}>
                          <span />
                          <span />
                          <span />
                          <span />
                        </div>
                      </div>

                      {media.durationSec > 0 && (
                        <div style={{ marginTop: 12 }}>
                          <input
                            type="range"
                            className="notch-scrubber"
                            min={0}
                            max={Math.max(1, media.durationSec)}
                            step={1}
                            value={Math.min(media.positionSec, media.durationSec)}
                            onChange={(e) => {
                              const next = Number(e.target.value);
                              setMedia((prev) => ({ ...prev, positionSec: next }));
                              invoke("seek_media", { positionSec: next }).catch(() => {});
                            }}
                          />
                          <div style={{ display: "flex", justifyContent: "space-between", fontSize: 11, color: "#94a3b8", fontVariantNumeric: "tabular-nums" }}>
                            <span>{fmtTime(media.positionSec)}</span>
                            <span>-{fmtTime(Math.max(0, media.durationSec - media.positionSec))}</span>
                          </div>
                        </div>
                      )}

                      <div style={{ display: "flex", justifyContent: "center", alignItems: "center", gap: 14, marginTop: 10 }}>
                        <button className="icon-btn" onClick={() => invoke("control_media", { action: "prev" })}>
                          <SkipBack size={16} />
                        </button>
                        <button
                          className="icon-btn"
                          style={{ padding: "8px 14px", background: "rgba(255,255,255,0.16)" }}
                          onClick={() => invoke("control_media", { action: "toggle" })}
                        >
                          {media.isPlaying ? <Pause size={17} /> : <Play size={17} />}
                        </button>
                        <button className="icon-btn" onClick={() => invoke("control_media", { action: "next" })}>
                          <SkipForward size={16} />
                        </button>
                      </div>
                    </>
                  ) : (
                    <div style={{ padding: "24px 8px", textAlign: "center", color: "#94a3b8" }}>
                      <Music size={22} style={{ marginBottom: 6, opacity: 0.7 }} />
                      <div>{STRINGS.noMedia}</div>
                    </div>
                  )}
                </div>
              )}

              {tab === "shelf" && (
                <div
                  onDragOver={(e) => e.preventDefault()}
                  onDrop={(e) => {
                    e.preventDefault();
                    const dropped = Array.from(e.dataTransfer.files).map((f) => ({
                      name: f.name,
                      size: f.size,
                    }));
                    setShelf((prev) => [...prev, ...dropped]);
                  }}
                  style={{
                    border: "1px dashed rgba(255,255,255,0.22)",
                    borderRadius: 14,
                    padding: 18,
                    textAlign: "center",
                    color: "#94a3b8",
                  }}
                >
                  {shelf.length === 0 ? (
                    <div style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 6 }}>
                      <FolderOpen size={22} color="#38bdf8" />
                      <div style={{ color: "#e2e8f0", fontWeight: 600 }}>File Shelf is Empty</div>
                      <div style={{ fontSize: 12 }}>Drag files from File Explorer to the Notch to hold them.</div>
                    </div>
                  ) : (
                    <div style={{ display: "flex", flexDirection: "column", gap: 6, textAlign: "left", maxHeight: 168, overflowY: "auto" }}>
                      {shelf.map((f, i) => (
                        <div key={`${f.name}-${i}`} style={{ display: "flex", alignItems: "center", gap: 8, color: "#f8fafc" }}>
                          <FileText size={14} />
                          <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>{f.name}</span>
                          <button
                            className="icon-btn"
                            onClick={() => setShelf((prev) => prev.filter((_, idx) => idx !== i))}
                          >
                            <Trash2 size={12} />
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}

              {tab === "timer" && (
                <div>
                  <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", gap: 14 }}>
                    <div style={{ display: "flex", alignItems: "center", gap: 14 }}>
                      <svg width={82} height={82} viewBox="0 0 82 82">
                        <circle cx={41} cy={41} r={ringRadius} fill="none" stroke="rgba(255,255,255,0.12)" strokeWidth={6} />
                        <circle
                          cx={41}
                          cy={41}
                          r={ringRadius}
                          fill="none"
                          stroke="#f59e0b"
                          strokeWidth={6}
                          strokeLinecap="round"
                          strokeDasharray={ringCircumference}
                          strokeDashoffset={ringOffset}
                          transform="rotate(-90 41 41)"
                        />
                        <text
                          x={41}
                          y={45}
                          textAnchor="middle"
                          fill="#f8fafc"
                          fontSize="14"
                          fontWeight="700"
                          style={{ fontVariantNumeric: "tabular-nums" }}
                        >
                          {fmtTime(timerSec)}
                        </text>
                      </svg>
                      <div>
                        <div style={{ fontWeight: 700, fontSize: 15 }}>{timerMinutes}m Focus</div>
                        <div style={{ fontSize: 12, color: "#94a3b8" }}>
                          {timerRunning ? "Running" : timerSec === 0 ? "Completed" : "Ready to start"}
                        </div>
                      </div>
                    </div>
                    <div style={{ display: "flex", gap: 6 }}>
                      <button className="icon-btn" onClick={() => setTimerRunning((v) => !v)}>
                        {timerRunning ? <Pause size={15} /> : <Play size={15} />}
                      </button>
                      <button
                        className="icon-btn"
                        onClick={() => {
                          setTimerRunning(false);
                          setTimerSec(timerMinutes * 60);
                        }}
                      >
                        <RotateCcw size={15} />
                      </button>
                    </div>
                  </div>
                  <div style={{ display: "flex", gap: 6, marginTop: 12 }}>
                    {TIMER_PRESETS.map((m) => (
                      <button
                        key={m}
                        className={`notch-chip ${timerMinutes === m ? "active" : ""}`}
                        onClick={() => {
                          setTimerRunning(false);
                          setTimerMinutes(m);
                          setTimerSec(m * 60);
                        }}
                      >
                        {m}m
                      </button>
                    ))}
                  </div>
                </div>
              )}

              {tab === "clipboard" && (
                <div>
                  <div style={{ display: "flex", alignItems: "center", gap: 6, marginBottom: 8 }}>
                    <div className="notch-search-box">
                      <Search size={12} color="#94a3b8" />
                      <input
                        value={clipQuery}
                        onChange={(e) => setClipQuery(e.target.value)}
                        placeholder="Search history..."
                      />
                    </div>
                    <button
                      className={`notch-chip ${clipFilter === "all" ? "active" : ""}`}
                      onClick={() => setClipFilter("all")}
                    >
                      All
                    </button>
                    <button
                      className={`notch-chip ${clipFilter === "text" ? "active" : ""}`}
                      onClick={() => setClipFilter("text")}
                    >
                      Text
                    </button>
                    <button
                      className={`notch-chip ${clipFilter === "image" ? "active" : ""}`}
                      onClick={() => setClipFilter("image")}
                    >
                      Images
                    </button>
                    <button
                      className="notch-chip"
                      onClick={() => invoke<ClipboardItem[]>("clear_clipboard_history").then(setClips)}
                    >
                      Clear
                    </button>
                  </div>

                  {filteredClips.length === 0 ? (
                    <div style={{ padding: "22px 8px", textAlign: "center", color: "#94a3b8" }}>
                      <Clipboard size={20} style={{ marginBottom: 6, opacity: 0.7 }} />
                      <div style={{ color: "#e2e8f0", fontWeight: 600, fontSize: 13 }}>Clipboard History is empty</div>
                      <div style={{ fontSize: 11 }}>Copy text, links, or screenshots to capture them here.</div>
                    </div>
                  ) : (
                    <div style={{ maxHeight: 158, overflowY: "auto", display: "flex", flexDirection: "column", gap: 5 }}>
                      {filteredClips.slice(0, 14).map((c) => (
                        <div key={c.id} className="clip-row">
                          <button
                            className="clip-row-btn"
                            title="Click to copy back to Clipboard"
                            onClick={() => {
                              invoke("copy_clipboard_item", {
                                text: c.content,
                                imageDataUrl: c.imageDataUrl ?? null,
                              }).then(() => {
                                setCopiedId(c.id);
                                window.setTimeout(() => setCopiedId((prev) => (prev === c.id ? null : prev)), 1100);
                              });
                            }}
                          >
                            {c.imageDataUrl && (
                              <img
                                src={c.imageDataUrl}
                                alt=""
                                style={{ width: 28, height: 28, borderRadius: 6, objectFit: "cover", flexShrink: 0 }}
                              />
                            )}
                            <div style={{ flex: 1, minWidth: 0 }}>
                              <div style={{ fontSize: 12, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                                {c.content}
                              </div>
                              <div style={{ fontSize: 10, color: "#94a3b8" }}>
                                {new Date(c.timestampMs).toLocaleTimeString([], { hour: "numeric", minute: "2-digit" })} ·{" "}
                                {c.kind === "image" ? "Image" : c.kind === "url" ? "Link" : "Text"}
                              </div>
                            </div>
                          </button>
                          {copiedId === c.id && <Check size={13} color="#34d399" />}
                          <button
                            className="icon-btn"
                            style={{ padding: 5 }}
                            onClick={() =>
                              invoke<ClipboardItem[]>("toggle_pin_clipboard_item", { id: c.id }).then(setClips)
                            }
                          >
                            <Pin size={12} color={c.pinned ? "#38bdf8" : "#94a3b8"} />
                          </button>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
