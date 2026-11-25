use crate::config::AppConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct IcsEvent {
    pub summary: String,
    pub dtstart: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WidgetRect {
    pub x: i32,
    pub y: i32,
    pub w: i32,
    pub h: i32,
}

/// Magnetic snap aligns dragged widget edges and standard 16px gaps against sibling cards.
pub fn snap_widget_position(dragged: WidgetRect, others: &[WidgetRect], threshold: i32) -> (i32, i32) {
    let mut best_x = dragged.x;
    let mut best_y = dragged.y;
    let mut dx_min = threshold + 1;
    let mut dy_min = threshold + 1;
    const GAP: i32 = 16;

    for o in others {
        let x_candidates = [o.x, o.x + o.w + GAP, o.x - dragged.w - GAP, o.x + o.w - dragged.w];
        for cx in x_candidates {
            let d = (dragged.x - cx).abs();
            if d < dx_min {
                dx_min = d;
                best_x = cx;
            }
        }
        let y_candidates = [o.y, o.y + o.h + GAP, o.y - dragged.h - GAP, o.y + o.h - dragged.h];
        for cy in y_candidates {
            let d = (dragged.y - cy).abs();
            if d < dy_min {
                dy_min = d;
                best_y = cy;
            }
        }
    }
    (best_x, best_y)
}

pub fn parse_https_url(url: &str) -> Option<(String, String)> {
    let trimmed = url.trim();
    let rest = trimmed.strip_prefix("https://")?;
    if rest.is_empty() {
        return None;
    }
    let (host, path) = rest.split_once('/').unwrap_or((rest, ""));
    let host = host.trim();
    if host.is_empty() || host.contains(char::is_whitespace) {
        return None;
    }
    Some((host.to_string(), format!("/{}", path)))
}

/// Parses VEVENT SUMMARY and DTSTART fields from an iCalendar (.ics) payload, unfolding RFC 5545 folded lines.
pub fn parse_ics_events(ics_text: &str) -> Vec<IcsEvent> {
    let mut unfolded = Vec::<String>::new();
    for raw in ics_text.lines() {
        let line = raw.trim_end_matches('\r');
        if let Some(cont) = line.strip_prefix(' ').or_else(|| line.strip_prefix('\t')) {
            if let Some(last) = unfolded.last_mut() {
                last.push_str(cont);
                continue;
            }
        }
        unfolded.push(line.to_string());
    }

    let mut events = Vec::new();
    let mut in_event = false;
    let mut summary = String::new();
    let mut dtstart = String::new();

    for raw in unfolded {
        let line = raw.trim();
        if line.eq_ignore_ascii_case("BEGIN:VEVENT") {
            in_event = true;
            summary.clear();
            dtstart.clear();
        } else if line.eq_ignore_ascii_case("END:VEVENT") {
            if in_event && !summary.is_empty() {
                events.push(IcsEvent {
                    summary: summary.clone(),
                    dtstart: dtstart.clone(),
                });
            }
            in_event = false;
        } else if in_event {
            if let Some((prop, val)) = line.split_once(':') {
                let name = prop.split(';').next().unwrap_or("").trim();
                if name.eq_ignore_ascii_case("SUMMARY") {
                    summary = val
                        .trim()
                        .replace("\\n", " ")
                        .replace("\\,", ",")
                        .replace("\\;", ";");
                } else if name.eq_ignore_ascii_case("DTSTART") {
                    dtstart = val.trim().to_string();
                }
            }
        }
    }
    events.sort_by(|a, b| a.dtstart.cmp(&b.dtstart));
    events.truncate(20);
    events
}

pub fn sync_cards(cfg: &AppConfig) {
    // Native card window layout coordinates for Vinyl (332px), Calendar (332px), and Memo (332px)
    let mut rects = Vec::new();
    if cfg.vinyl_enabled {
        rects.push(WidgetRect { x: 48, y: 120, w: 332, h: 148 });
    }
    if cfg.calendar_enabled {
        let base = WidgetRect { x: 48, y: 284, w: 332, h: 240 };
        let (sx, sy) = snap_widget_position(base, &rects, 14);
        rects.push(WidgetRect { x: sx, y: sy, ..base });
    }
    if cfg.memo_enabled {
        let base = WidgetRect { x: 48, y: 540, w: 332, h: 200 };
        let (sx, sy) = snap_widget_position(base, &rects, 14);
        rects.push(WidgetRect { x: sx, y: sy, ..base });
    }
}
