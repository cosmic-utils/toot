use std::collections::HashMap;

use cosmic::{
    iced::core::image,
    widget::{self, image::Handle},
};
use megalodon::entities::{Notification, Status};

use crate::error::Error;

#[derive(Debug, Clone)]
pub struct Cache {
    pub handles: HashMap<String, Handle>,
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

    pub fn insert_handle(&mut self, url: String, handle: Handle) {
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
