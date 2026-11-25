import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AppWindow, Calculator, Search } from "lucide-react";
import { AppConfig, SpotlightSearchResult } from "../types";
import { STRINGS } from "../i18n";

export function SpotlightWindow() {
  const [query, setQuery] = useState("");
  const [res, setRes] = useState<SpotlightSearchResult>({ entries: [] });
  const [selected, setSelected] = useState(0);
  const [theme, setTheme] = useState("dark");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    invoke<AppConfig>("get_config").then((c) => setTheme(c.theme)).catch(() => {});
    const unlistenCfg = listen<AppConfig>("antibar://config-changed", (e) => setTheme(e.payload.theme));
    const unlisten = listen("antibar://spotlight-opened", () => {
      setQuery("");
      setSelected(0);
      setTimeout(() => inputRef.current?.focus(), 40);
    });
    return () => {
      unlistenCfg.then((u) => u());
      unlisten.then((u) => u());
    };
  }, []);

  useEffect(() => {
    invoke<SpotlightSearchResult>("search_spotlight", { query })
      .then((r) => {
        setRes(r);
        setSelected(0);
      })
      .catch(() => {});
  }, [query]);

  const handleLaunch = async (idx: number) => {
    const entry = res.entries[idx];
    if (!entry) return;
    await invoke("launch_app", { path: entry.path, args: null });
    await invoke("hide_spotlight");
  };

  const formattedMath = (() => {
    if (!res.mathAnswer) return null;
    const n = Number(res.mathAnswer);
    return Number.isFinite(n) ? n.toLocaleString(undefined, { maximumFractionDigits: 6 }) : res.mathAnswer;
  })();

  return (
    <div
      className={`spotlight-wrap theme-${theme}`}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          invoke("hide_spotlight");
        } else if (e.key === "ArrowDown") {
          e.preventDefault();
          setSelected((s) => Math.min(s + 1, Math.max(0, res.entries.length - 1)));
        } else if (e.key === "ArrowUp") {
          e.preventDefault();
          setSelected((s) => Math.max(s - 1, 0));
        } else if (e.key === "Enter") {
          e.preventDefault();
          handleLaunch(selected);
        }
      }}
    >
      <div className="spotlight-card">
        <div className="spotlight-input-row">
          <Search size={20} color="#0ea5e9" />
          <input
            ref={inputRef}
            autoFocus
            className="spotlight-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={STRINGS.spotlightPlaceholder}
          />
          <button
            className="spotlight-kbd-btn"
            onClick={() => invoke("hide_spotlight")}
            title="Close Spotlight"
          >
            Esc close
          </button>
        </div>

        <div className="spotlight-results">
          {formattedMath && (
            <div className="spotlight-row" style={{ background: "rgba(14,165,233,0.14)", marginBottom: 6 }}>
              <Calculator size={20} color="#0ea5e9" />
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 18, fontWeight: 700 }}>= {formattedMath}</div>
                <div className="muted-label" style={{ fontSize: 12 }}>{query}</div>
              </div>
            </div>
          )}

          {res.entries.map((entry, idx) => {
            const isSel = idx === selected;
            return (
              <button
                key={entry.id}
                className={`spotlight-row ${isSel ? "selected" : ""}`}
                onMouseEnter={() => setSelected(idx)}
                onClick={() => handleLaunch(idx)}
              >
                {entry.iconDataUrl ? (
                  <img src={entry.iconDataUrl} alt="" style={{ width: 24, height: 24, objectFit: "contain" }} />
                ) : (
                  <AppWindow size={22} className="muted-label" />
                )}
                <div style={{ flex: 1, minWidth: 0 }}>
                  <div style={{ fontSize: 14, fontWeight: 500, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {entry.name}
                  </div>
                  <div className="muted-label" style={{ fontSize: 12, overflow: "hidden", textOverflow: "ellipsis", whiteSpace: "nowrap" }}>
                    {entry.subtitle}
                  </div>
                </div>
                {isSel && <span className="spotlight-open-pill">Open ↵</span>}
              </button>
            );
          })}
        </div>
      </div>
    </div>
  );
}
