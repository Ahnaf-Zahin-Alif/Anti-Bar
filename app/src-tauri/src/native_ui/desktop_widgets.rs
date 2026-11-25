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

/// Parses VEVENT SUMMARY and DTSTART fields from an iCalendar (.ics) payload.
pub fn parse_ics_events(ics_text: &str) -> Vec<IcsEvent> {
    let mut events = Vec::new();
    let mut in_event = false;
    let mut summary = String::new();
    let mut dtstart = String::new();

    for raw in ics_text.lines() {
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
            if let Some(val) = line.strip_prefix("SUMMARY:") {
                summary = val.trim().to_string();
            } else if let Some((_, val)) = line.split_once(':') {
                if line.starts_with("DTSTART") {
                    dtstart = val.trim().to_string();
                }
            }
        }
    }
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
