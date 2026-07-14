// SPDX-License-Identifier: {{LICENSE}}

use error::Error;

mod app;
mod config;
mod error;
mod i18n;
mod mastodon;
mod pages;
mod settings;
mod subscriptions;
mod utils;
mod widgets;

fn main() -> Result<(), Error> {
    settings::init();
    cosmic::app::run::<app::AppModel>(settings::settings(), settings::flags()).map_err(Error::Iced)
}
