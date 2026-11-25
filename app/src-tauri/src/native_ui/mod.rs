pub mod desktop_clock;
pub mod desktop_widgets;
pub mod genie;

use crate::config::AppConfig;

pub fn sync_native_widgets(cfg: &AppConfig) {
    desktop_clock::sync_clock_widget(cfg);
    desktop_widgets::sync_cards(cfg);
    genie::set_genie_enabled(cfg.genie_effect);
}
