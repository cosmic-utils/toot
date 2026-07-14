// SPDX-License-Identifier: {{LICENSE}}

use cosmic::{
    cosmic_config::{self, cosmic_config_derive::CosmicConfigEntry, Config, CosmicConfigEntry},
    Application,
};

use crate::app::AppModel;

#[derive(Debug, Default, Clone, CosmicConfigEntry, Eq, PartialEq)]
#[version = 1]
pub struct TootConfig {
    pub server: String,
    /// Hide boosted (reblogged) statuses from timelines.
    pub hide_boosts: bool,
    /// Hide reply statuses from timelines.
    pub hide_replies: bool,
    /// How much of each post to render in feeds.
    pub feed_density: FeedDensity,
    /// Preferred theme.
    pub theme_mode: ThemeMode,
}

/// The user's preferred theme.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum ThemeMode {
    Light,
    Dark,
    #[default]
    System,
}

impl ThemeMode {
    pub const ALL: [ThemeMode; 3] = [ThemeMode::Light, ThemeMode::Dark, ThemeMode::System];

    pub fn label(self) -> &'static str {
        match self {
            ThemeMode::Light => "Light",
            ThemeMode::Dark => "Dark",
            ThemeMode::System => "System",
        }
    }

    pub fn theme(self) -> cosmic::Theme {
        match self {
            ThemeMode::Light => cosmic::Theme::light(),
            ThemeMode::Dark => cosmic::Theme::dark(),
            ThemeMode::System => cosmic::theme::system_preference(),
        }
    }
}

/// How much of each post to render in feeds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum FeedDensity {
    /// Just the header and text content — no card, media, or tags.
    TextOnly,
    /// Card, media, and tags too, but shown smaller.
    Compact,
    /// Everything, at full size.
    #[default]
    Full,
}

impl FeedDensity {
    pub const ALL: [FeedDensity; 3] = [
        FeedDensity::TextOnly,
        FeedDensity::Compact,
        FeedDensity::Full,
    ];

    pub fn label(self) -> &'static str {
        match self {
            FeedDensity::TextOnly => "Text only",
            FeedDensity::Compact => "Compact",
            FeedDensity::Full => "Full",
        }
    }
}

impl TootConfig {
    pub fn config_handler() -> Option<Config> {
        Config::new(AppModel::APP_ID, TootConfig::VERSION).ok()
    }

    pub fn config() -> TootConfig {
        match Self::config_handler() {
            Some(config_handler) => {
                TootConfig::get_entry(&config_handler).unwrap_or_else(|(errs, config)| {
                    tracing::error!("errors loading config: {:?}", errs);
                    config
                })
            }
            None => TootConfig::default(),
        }
    }
}
