import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Clipboard,
  FileText,
  Music,
  Pause,
  Pin,
  Play,
  SkipBack,
  SkipForward,
  Timer,
  Trash2,
} from "lucide-react";
import { AppConfig, ClipboardItem, LyricsResult, MediaState } from "../types";
import { resolveLang, STRINGS } from "../i18n";

type NotchTab = "media" | "shelf" | "clipboard" | "timer";

interface ShelfFile {
  name: string;
  size: number;
}

export function NotchWindow() {
  const [config, setConfig] = useState<AppConfig | null>(null);
  const [expanded, setExpanded] = useState(false);
  const [tab, setTab] = useState<NotchTab>("media");
  const [media, setMedia] = useState<MediaState>({
    title: "",
    artist: "",
    album: "",
    isPlaying: false,
    positionSec: 0,
    durationSec: 0,
    hasSession: false,
  });
  const [lyrics, setLyrics] = useState<LyricsResult | null>(null);
  const [clips, setClips] = useState<ClipboardItem[]>([]);
  const [shelf, setShelf] = useState<ShelfFile[]>([]);
  const [timerSec, setTimerSec] = useState(25 * 60);
  const [timerRunning, setTimerRunning] = useState(false);
  const lastTrackRef = useRef("");

  useEffect(() => {
    invoke<AppConfig>("get_config").then(setConfig).catch(() => {});
    invoke<ClipboardItem[]>("get_clipboard_history").then(setClips).catch(() => {});
    invoke<MediaState>("get_media_state").then(setMedia).catch(() => {});

    const unlistenMedia = listen<MediaState>("antibar://media-updated", (e) => {
      setMedia(e.payload);
    });
    const unlistenClip = listen<ClipboardItem[]>("antibar://clipboard-updated", (e) => {
      setClips(e.payload);
    });
    const unlistenCfg = listen<AppConfig>("antibar://config-changed", (e) => setConfig(e.payload));

    return () => {
      unlistenMedia.then((u) => u());
      unlistenClip.then((u) => u());
      unlistenCfg.then((u) => u());
    };
  }, []);

  useEffect(() => {
    if (!config?.showLyrics || !media.title) return;
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
      setTimerSec((s) => (s > 0 ? s - 1 : 0));
    }, 1000);
    return () => clearInterval(id);
  }, [timerRunning]);

  useEffect(() => {
    invoke("resize_overlay", {
      label: "notch",
      width: expanded ? 460 : 270,
      height: expanded ? 320 : 52,
    }).catch(() => {});
  }, [expanded]);

  const lang = resolveLang(config?.language ?? "auto");
  const t = STRINGS[lang];

  const activeLyric =
    [...(lyrics?.synced ?? [])].reverse().find((l) => l.timeSec <= media.positionSec)?.text ?? "";

  const fmtTime = (sec: number) => {
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    return `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;
  };

  return (
    <div className="notch-shell">
      <div
        className={`notch-pill ${expanded ? "expanded" : ""}`}
        onMouseLeave={() => setExpanded(false)}
      >
        <div className="notch-compact-row" onClick={() => setExpanded((v) => !v)}>
          <div style={{ display: "flex", alignItems: "center", gap: 8, minWidth: 0 }}>
            <Music size={14} color="#38bdf8" />
            <span style={{ overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
              {media.hasSession && media.title ? media.title : "Anti-Bar"}
            </span>
          </div>
          <span style={{ color: "#94a3b8", fontVariantNumeric: "tabular-nums" }}>
            {timerRunning ? fmtTime(timerSec) : new Date().toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}
          </span>
        </div>

        {expanded && (
          <>
            <div className="notch-tabs">
              <button className={`notch-tab ${tab === "media" ? "active" : ""}`} onClick={() => setTab("media")}>
                {t.mediaTab}
              </button>
              <button className={`notch-tab ${tab === "shelf" ? "active" : ""}`} onClick={() => setTab("shelf")}>
                {t.shelfTab}
              </button>
              <button className={`notch-tab ${tab === "clipboard" ? "active" : ""}`} onClick={() => setTab("clipboard")}>
                {t.clipboardTab}
              </button>
              <button className={`notch-tab ${tab === "timer" ? "active" : ""}`} onClick={() => setTab("timer")}>
                {t.timerTab}
              </button>
            </div>

            <div className="notch-body">
              {tab === "media" && (
                <div>
                  {media.hasSession && media.title ? (
                    <>
                      <div style={{ fontWeight: 600 }}>{media.title}</div>
                      <div style={{ fontSize: 12, color: "#94a3b8", marginBottom: 8 }}>{media.artist}</div>
                      {activeLyric && (
                        <div style={{ fontSize: 12, color: "#38bdf8", marginBottom: 8 }}>{activeLyric}</div>
                      )}
                      <div style={{ display: "flex", gap: 8 }}>
                        <button className="icon-btn" onClick={() => invoke("control_media", { action: "prev" })}>
                          <SkipBack size={15} />
                        </button>
                        <button className="icon-btn" onClick={() => invoke("control_media", { action: "toggle" })}>
                          {media.isPlaying ? <Pause size={15} /> : <Play size={15} />}
                        </button>
                        <button className="icon-btn" onClick={() => invoke("control_media", { action: "next" })}>
                          <SkipForward size={15} />
                        </button>
                      </div>
                    </>
                  ) : (
                    <div style={{ color: "#94a3b8" }}>{t.noMedia}</div>
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
                    borderRadius: 12,
                    padding: 14,
                    textAlign: "center",
                    color: "#94a3b8",
                  }}
                >
                  {shelf.length === 0 ? (
                    <div>{t.dropHint}</div>
                  ) : (
                    <div style={{ display: "flex", flexDirection: "column", gap: 6, textAlign: "left" }}>
                      {shelf.map((f, i) => (
                        <div key={`${f.name}-${i}`} style={{ display: "flex", alignItems: "center", gap: 8, color: "#f8fafc" }}>
                          <FileText size={14} />
                          <span style={{ flex: 1, overflow: "hidden", textOverflow: "ellipsis" }}>{f.name}</span>
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

              {tab === "clipboard" && (
                <div>
                  <div style={{ display: "flex", justifyContent: "space-between", marginBottom: 6 }}>
                    <span style={{ fontSize: 12, color: "#94a3b8" }}>
                      <Clipboard size={12} style={{ verticalAlign: "middle", marginRight: 4 }} />
                      {clips.length} items
                    </span>
                    <button
                      className="icon-btn"
                      style={{ fontSize: 11, padding: "3px 8px" }}
                      onClick={() => invoke<ClipboardItem[]>("clear_clipboard_history").then(setClips)}
                    >
                      {t.clearUnpinned}
                    </button>
                  </div>
                  <div style={{ maxHeight: 150, overflowY: "auto", display: "flex", flexDirection: "column", gap: 4 }}>
                    {clips.slice(0, 12).map((c) => (
                      <div
                        key={c.id}
                        style={{
                          display: "flex",
                          alignItems: "center",
                          gap: 6,
                          padding: "5px 8px",
                          borderRadius: 8,
                          background: "rgba(255,255,255,0.06)",
                        }}
                      >
                        <button
                          onClick={() => invoke("copy_clipboard_item", { text: c.content })}
                          style={{
                            flex: 1,
                            background: "transparent",
                            border: "none",
                            color: "#f8fafc",
                            textAlign: "left",
                            overflow: "hidden",
                            textOverflow: "ellipsis",
                            whiteSpace: "nowrap",
                            cursor: "pointer",
                          }}
                        >
                          {c.content}
                        </button>
                        <button
                          className="icon-btn"
                          onClick={() =>
                            invoke<ClipboardItem[]>("toggle_pin_clipboard_item", { id: c.id }).then(setClips)
                          }
                        >
                          <Pin size={12} color={c.pinned ? "#38bdf8" : "#94a3b8"} />
                        </button>
                      </div>
                    ))}
                  </div>
                </div>
              )}

              {tab === "timer" && (
                <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}>
                  <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                    <Timer size={18} color="#38bdf8" />
                    <span style={{ fontSize: 22, fontWeight: 700, fontVariantNumeric: "tabular-nums" }}>
                      {fmtTime(timerSec)}
                    </span>
                  </div>
                  <div style={{ display: "flex", gap: 6 }}>
                    <button className="icon-btn" onClick={() => setTimerRunning((v) => !v)}>
                      {timerRunning ? <Pause size={15} /> : <Play size={15} />}
                    </button>
                    <button
                      className="icon-btn"
                      onClick={() => {
                        setTimerRunning(false);
                        setTimerSec(25 * 60);
                      }}
                    >
                      25m
                    </button>
                  </div>
                </div>
              )}
            </div>
          </>
        )}
      </div>
    </div>
  );
}
