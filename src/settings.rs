use cosmic::{app::Settings, iced::Limits};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use crate::{app::Flags, config::TootConfig, i18n};

pub fn init() {
    let requested_languages = i18n_embed::DesktopLanguageRequester::requested_languages();
    i18n::init(&requested_languages);

    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("{}=debug", env!("CARGO_CRATE_NAME")).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();
}

pub fn settings() -> Settings {
    let theme = TootConfig::config().theme_mode.theme();
    Settings::default()
        .size_limits(Limits::NONE.min_width(360.0).min_height(180.0))
        .theme(theme)
}

pub fn flags() -> Flags {
    let (config, handler) = (TootConfig::config(), TootConfig::config_handler());
    Flags { config, handler }
}
