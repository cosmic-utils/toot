use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Mastodon API error: {0}")]
    Mastodon(#[from] megalodon::error::Error),
    #[error("Iced error: {0}")]
    Iced(#[from] cosmic::iced::Error),
    #[error("Reqwest error: {0}")]
    Reqwest(#[from] reqwest::Error),
}
