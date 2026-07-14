use std::collections::HashMap;

use cosmic::{
    iced::core::image,
    widget::{self, image::Handle},
};
use megalodon::entities::{Account, Notification, Relationship, Status};

use crate::config::FeedDensity;
use crate::error::Error;

#[derive(Debug, Clone)]
pub struct Cache {
    pub handles: HashMap<String, Handle>,
    pub statuses: HashMap<String, Status>,
    pub notifications: HashMap<String, Notification>,
    /// The authenticated user's relationship (following/muting/blocking) to
    /// each account whose profile has been viewed, keyed by account id.
    pub relationships: HashMap<String, Relationship>,
    /// The currently authenticated account, if logged in. Used to decide
    /// which statuses/relationships belong to the current user (e.g. to
    /// show a delete action only on your own posts).
    pub me: Option<Account>,
    /// Timeline display preferences from [`crate::config::TootConfig`],
    /// snapshotted here since it's already threaded through every feature's
    /// `view(&Cache)` call.
    pub hide_boosts: bool,
    pub hide_replies: bool,
    pub feed_density: FeedDensity,
    /// Set whenever new content is cached; cleared once flushed to disk.
    /// Lets the periodic save subscription skip writing when nothing changed.
    pub dirty: bool,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
            statuses: HashMap::new(),
            notifications: HashMap::new(),
            relationships: HashMap::new(),
            me: None,
            hide_boosts: false,
            hide_replies: false,
            feed_density: FeedDensity::default(),
            dirty: false,
        }
    }

    /// Whether a status should be shown given the current display preferences.
    pub fn is_visible(&self, status: &Status) -> bool {
        !(self.hide_boosts && status.reblog.is_some()
            || self.hide_replies && status.in_reply_to_id.is_some())
    }

    pub fn insert_relationship(&mut self, relationship: Relationship) {
        self.relationships
            .insert(relationship.id.clone(), relationship);
    }

    /// Whether the given account id belongs to the authenticated user.
    pub fn is_me(&self, account_id: &str) -> bool {
        self.me.as_ref().is_some_and(|account| account.id == account_id)
    }

    pub fn insert_status(&mut self, status: Status) {
        self.statuses.insert(status.id.to_string(), status.clone());
        if let Some(reblog) = status.reblog {
            self.statuses.insert(reblog.id.to_string(), *reblog);
        }
        self.dirty = true;
    }

    pub fn insert_notification(&mut self, notification: Notification) {
        self.notifications
            .insert(notification.id.to_string(), notification.clone());
        if let Some(status) = notification.status {
            self.insert_status(status.clone());
        }
        self.dirty = true;
    }

    pub fn insert_handle(&mut self, url: String, handle: Handle) {
        self.handles.insert(url, handle);
    }

    pub fn clear(&mut self) {
        self.statuses.clear();
        self.notifications.clear();
        self.handles.clear();
        self.relationships.clear();
        self.me = None;
        self.dirty = false;
    }
}

pub fn fallback_avatar<'a>() -> widget::Image<'a> {
    widget::image(image::Handle::from_bytes(
        include_bytes!("../assets/missing.png").to_vec(),
    ))
}

pub fn fallback_handle() -> widget::image::Handle {
    image::Handle::from_bytes(include_bytes!("../assets/missing.png").to_vec())
}

pub async fn get(url: impl ToString) -> Result<Handle, Error> {
    let url = url.to_string();

    if let Some(bytes) = load_cached_image(url.clone()).await {
        return Ok(Handle::from_bytes(bytes));
    }

    let response = reqwest::get(&url).await?;
    match response.error_for_status() {
        Ok(response) => {
            let bytes = response.bytes().await?.to_vec();
            save_cached_image(url, bytes.clone());
            Ok(Handle::from_bytes(bytes))
        }
        Err(err) => Err(err.into()),
    }
}

/// Disk reads/writes are blocking; run them on tokio's blocking pool so a
/// cold or growing image cache doesn't stall the async executor (and with
/// it, every other in-flight fetch) one file at a time.
async fn load_cached_image(url: String) -> Option<Vec<u8>> {
    tokio::task::spawn_blocking(move || crate::persistence::load_image(&url))
        .await
        .ok()
        .flatten()
}

fn save_cached_image(url: String, bytes: Vec<u8>) {
    tokio::task::spawn_blocking(move || crate::persistence::save_image(&url, &bytes));
}

pub fn extract_status_images(status: &Status) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    if !status.account.avatar.is_empty() {
        urls.push(status.account.avatar.clone());
    }
    if !status.account.header.is_empty() {
        urls.push(status.account.header.clone());
    }

    if let Some(reblog) = &status.reblog {
        if !reblog.account.avatar.is_empty() {
            urls.push(reblog.account.avatar.clone());
        }
        if !reblog.account.header.is_empty() {
            urls.push(reblog.account.header.clone());
        }
        if let Some(card) = &reblog.card {
            if let Some(image) = &card.image {
                urls.push(image.clone());
            }
        }
        for attachment in &reblog.media_attachments {
            if let Some(url) = &attachment.preview_url {
                urls.push(url.clone());
            }
        }
    }

    if let Some(card) = &status.card {
        if let Some(image) = &card.image {
            urls.push(image.clone());
        }
    }

    for attachment in &status.media_attachments {
        if let Some(url) = &attachment.preview_url {
            urls.push(url.clone());
        }
    }

    urls
}

pub fn extract_notification_images(notification: &Notification) -> Vec<String> {
    let mut urls: Vec<String> = Vec::new();
    if let Some(account) = &notification.account {
        if !account.avatar.is_empty() {
            urls.push(account.avatar.clone());
        }
        if !account.header.is_empty() {
            urls.push(account.header.clone());
        }
    }

    if let Some(status) = &notification.status {
        if !status.account.avatar.is_empty() {
            urls.push(status.account.avatar.clone());
        }
        if !status.account.header.is_empty() {
            urls.push(status.account.header.clone());
        }
        if let Some(card) = &status.card {
            if let Some(image) = &card.image {
                urls.push(image.clone());
            }
        }
        for attachment in &status.media_attachments {
            if let Some(url) = &attachment.preview_url {
                urls.push(url.clone());
            }
        }
    }
    urls
}
