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
