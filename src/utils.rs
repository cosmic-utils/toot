use std::{collections::HashMap, str::FromStr};

use cosmic::{
    iced_core::image,
    widget::{self, image::Handle},
};
use mastodon_async::prelude::*;
use reqwest::Url;

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct Cache {
    pub handles: HashMap<Url, Handle>,
    pub statuses: HashMap<String, Status>,
    pub notifications: HashMap<String, Notification>,
}

impl Cache {
    pub fn new() -> Self {
        Self {
            handles: HashMap::new(),
            statuses: HashMap::new(),
            notifications: HashMap::new(),
        }
    }

    pub fn insert_status(&mut self, status: Status) {
        self.statuses.insert(status.id.to_string(), status.clone());
        if let Some(reblog) = status.reblog {
            self.statuses.insert(reblog.id.to_string(), *reblog);
        }
    }

    pub fn insert_notification(&mut self, notification: Notification) {
        self.notifications
            .insert(notification.id.to_string(), notification.clone());
        if let Some(status) = notification.status {
            self.insert_status(status.clone());
        }
    }

    pub fn insert_handle(&mut self, url: Url, handle: Handle) {
        self.handles.insert(url, handle);
    }

    #[allow(unused)]
    pub fn clear(&mut self) {
        self.statuses.clear();
        self.notifications.clear();
        self.handles.clear();
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
    let response = reqwest::get(url.to_string()).await?;
    match response.error_for_status() {
        Ok(response) => {
            let bytes = response.bytes().await?;
            let handle = Handle::from_bytes(bytes.to_vec());
            Ok(handle)
        }
        Err(err) => Err(err.into()),
    }
}

pub fn extract_status_images(status: &Status) -> Vec<Url> {
    let mut urls = Vec::new();
    urls.push(status.account.avatar.clone());
    urls.push(status.account.header.clone());

    if let Some(reblog) = &status.reblog {
        urls.push(reblog.account.avatar.clone());
        urls.push(reblog.account.header.clone());
        if let Some(card) = &reblog.card {
            if let Some(image) = &card.image {
                if let Ok(url) = Url::from_str(image) {
                    urls.push(url);
                }
            }
        }
        for attachment in &reblog.media_attachments {
            urls.push(attachment.preview_url.clone());
        }
    }

    if let Some(card) = &status.card {
        if let Some(image) = &card.image {
            if let Ok(url) = Url::from_str(image) {
                urls.push(url);
            }
        }
    }

    for attachment in &status.media_attachments {
        urls.push(attachment.preview_url.clone());
    }

    urls
}

pub fn extract_notification_images(notification: &Notification) -> Vec<Url> {
    let mut urls = Vec::new();
    urls.push(notification.account.avatar.clone());
    urls.push(notification.account.header.clone());

    if let Some(status) = &notification.status {
        urls.push(status.account.avatar.clone());
        urls.push(status.account.header.clone());
        if let Some(card) = &status.card {
            if let Some(image) = &card.image {
                if let Ok(url) = Url::from_str(image) {
                    urls.push(url);
                }
            }
        }
        for attachment in &status.media_attachments {
            urls.push(attachment.preview_url.clone());
        }
    }
    urls
}
