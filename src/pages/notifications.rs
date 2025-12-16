use std::collections::VecDeque;

use cosmic::{
    app::Task,
    iced::{Length, Subscription},
    iced_widget::scrollable::{Direction, Scrollbar},
    widget, Apply, Element,
};
use mastodon_async::{
    entities::notification::Notification,
    prelude::{Mastodon, NotificationId},
};

use crate::{
    app,
    utils::{self, Cache},
    widgets,
};

use super::MastodonPage;

#[derive(Debug, Clone)]
pub struct Notifications {
    pub mastodon: Mastodon,
    notifications: VecDeque<NotificationId>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Mastodon),
    AppendNotification(Notification),
    PrependNotification(Notification),
    Notification(crate::widgets::notification::Message),
}

impl MastodonPage for Notifications {
    fn is_authenticated(&self) -> bool {
        !self.mastodon.data.token.is_empty()
    }
}

impl Notifications {
    pub fn new(mastodon: Mastodon) -> Self {
        Self {
            mastodon,
            notifications: VecDeque::new(),
        }
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        let notifications: Vec<Element<_>> = self
            .notifications
            .iter()
            .filter_map(|id| cache.notifications.get(&id.to_string()))
            .map(|notification| {
                crate::widgets::notification(notification, cache).map(Message::Notification)
            })
            .collect();

        widget::scrollable(widget::settings::section().extend(notifications))
            .direction(Direction::Vertical(
                Scrollbar::default().spacing(spacing.space_xxs),
            ))
            .apply(widget::container)
            .max_width(700)
            .height(Length::Fill)
            .into()
    }

    pub fn update(&mut self, message: Message) -> Task<app::Message> {
        let mut tasks = vec![];
        match message {
            Message::SetClient(mastodon) => self.mastodon = mastodon,
            Message::AppendNotification(notification) => {
                self.notifications.push_back(notification.id.clone());
                tasks.push(cosmic::task::message(app::Message::CacheNotification(
                    notification.clone(),
                )));

                tasks.push(cosmic::task::message(app::Message::Fetch(
                    utils::extract_notification_images(&notification),
                )));
            }
            Message::PrependNotification(notification) => {
                self.notifications.push_front(notification.id.clone());
                tasks.push(cosmic::task::message(app::Message::CacheNotification(
                    notification,
                )));
            }
            Message::Notification(message) => match message {
                crate::widgets::notification::Message::Status(message) => {
                    tasks.push(widgets::status::update(message))
                }
            },
        }
        Task::batch(tasks)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.is_authenticated() && self.notifications.is_empty() {
            return Subscription::batch(vec![crate::subscriptions::notifications::timeline(
                self.mastodon.clone(),
            )]);
        }

        Subscription::none()
    }
}
