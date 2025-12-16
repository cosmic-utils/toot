use std::collections::VecDeque;

use cosmic::{
    app::Task,
    iced::{Length, Subscription},
    iced_widget::scrollable::{Direction, Scrollbar},
    widget, Apply, Element,
};
use mastodon_async::prelude::{Mastodon, Status, StatusId};

use crate::{
    app,
    utils::Cache,
    widgets::{self, status::StatusOptions},
};

use super::MastodonPage;

#[derive(Debug, Clone)]
pub struct Public {
    pub mastodon: Mastodon,
    statuses: VecDeque<StatusId>,
    timeline: TimelineType,
}

#[derive(Debug, Clone)]
pub enum TimelineType {
    Public,
    Local,
    Remote,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Mastodon),
    AppendStatus(Status),
    Status(crate::widgets::status::Message),
}

impl MastodonPage for Public {
    fn is_authenticated(&self) -> bool {
        !self.mastodon.data.token.is_empty()
    }
}

impl Public {
    pub fn new(mastodon: Mastodon, timeline: TimelineType) -> Self {
        Self {
            mastodon,
            statuses: VecDeque::new(),
            timeline,
        }
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        let statuses: Vec<Element<_>> = self
            .statuses
            .iter()
            .filter_map(|id| cache.statuses.get(&id.to_string()))
            .map(|status| {
                crate::widgets::status(status, StatusOptions::all(), cache).map(Message::Status)
            })
            .collect();

        widget::scrollable(widget::settings::section().extend(statuses))
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
            Message::AppendStatus(status) => {
                self.statuses.push_back(status.id.clone());
                tasks.push(cosmic::task::message(app::Message::CacheStatus(
                    status.clone(),
                )));

                tasks.push(cosmic::task::message(app::Message::Fetch(
                    crate::utils::extract_status_images(&status),
                )));
            }
            Message::Status(message) => tasks.push(widgets::status::update(message)),
        }
        Task::batch(tasks)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.statuses.is_empty() {
            return match self.timeline {
                TimelineType::Public => {
                    Subscription::batch(vec![crate::subscriptions::public::timeline(
                        self.mastodon.clone(),
                    )])
                }
                TimelineType::Local => {
                    Subscription::batch(vec![crate::subscriptions::public::local_timeline(
                        self.mastodon.clone(),
                    )])
                }
                TimelineType::Remote => {
                    Subscription::batch(vec![crate::subscriptions::public::remote_timeline(
                        self.mastodon.clone(),
                    )])
                }
            };
        }

        Subscription::none()
    }
}
