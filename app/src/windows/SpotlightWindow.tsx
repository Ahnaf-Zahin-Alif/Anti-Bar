import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { AppWindow, Calculator, Search } from "lucide-react";
import { AppConfig, SpotlightSearchResult } from "../types";
import { resolveLang, STRINGS } from "../i18n";

export function SpotlightWindow() {
  const [query, setQuery] = useState("");
  const [res, setRes] = useState<SpotlightSearchResult>({ entries: [] });
  const [selected, setSelected] = useState(0);
  const [langSetting, setLangSetting] = useState("auto");
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    invoke<AppConfig>("get_config").then((c) => setLangSetting(c.language)).catch(() => {});
    const unlisten = listen("antibar://spotlight-opened", () => {
      setQuery("");
      setSelected(0);
      setTimeout(() => inputRef.current?.focus(), 40);
    });
    return () => {
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

  const lang = resolveLang(langSetting);
  const t = STRINGS[lang];

  const handleLaunch = async (idx: number) => {
    const entry = res.entries[idx];
    if (!entry) return;
    await invoke("launch_app", { path: entry.path, args: null });
    await invoke("hide_spotlight");
  };

  return (
    <div
      className="spotlight-wrap"
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
          <Search size={20} color="#38bdf8" />
          <input
            ref={inputRef}
            autoFocus
            className="spotlight-input"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder={t.spotlightPlaceholder}
          />
        </div>

        <div className="spotlight-results">
          {res.mathAnswer && (
            <div className="spotlight-row" style={{ background: "rgba(14,165,233,0.12)", marginBottom: 6 }}>
              <Calculator size={20} color="#38bdf8" />
              <div>
                <div style={{ fontSize: 18, fontWeight: 700 }}>= {res.mathAnswer}</div>
                <div style={{ fontSize: 12, color: "#94a3b8" }}>{query}</div>
              </div>
            </div>
          )}

          {res.entries.map((entry, idx) => (
            <button
              key={entry.id}
              className={`spotlight-row ${idx === selected ? "selected" : ""}`}
              onClick={() => handleLaunch(idx)}
            >
              {entry.iconDataUrl ? (
                <img src={entry.iconDataUrl} alt="" style={{ width: 24, height: 24, objectFit: "contain" }} />
              ) : (
                <AppWindow size={22} color="#94a3b8" />
              )}
              <div style={{ flex: 1 }}>
                <div style={{ fontSize: 14, fontWeight: 500 }}>{entry.name}</div>
                <div style={{ fontSize: 12, color: "#94a3b8" }}>{entry.subtitle}</div>
              </div>
            </button>
          ))}
        </div>
      </div>
    </div>
  );
}
