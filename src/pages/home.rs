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
    utils::{self, Cache},
    widgets::{self, status::StatusOptions},
};

use super::MastodonPage;

#[derive(Debug, Clone)]
pub struct Home {
    pub mastodon: Mastodon,
    statuses: VecDeque<StatusId>,
    skip: usize,
    loading: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Mastodon),
    AppendStatus(Status),
    PrependStatus(Status),
    DeleteStatus(String),
    Status(crate::widgets::status::Message),
    LoadMore(bool),
}

impl MastodonPage for Home {
    fn is_authenticated(&self) -> bool {
        !self.mastodon.data.token.is_empty()
    }
}

impl Home {
    pub fn new(mastodon: Mastodon) -> Self {
        Self {
            mastodon,
            statuses: VecDeque::new(),
            skip: 0,
            loading: false,
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
            .on_scroll(|viewport| {
                Message::LoadMore(!self.loading && viewport.relative_offset().y == 1.0)
            })
            .apply(widget::container)
            .max_width(700)
            .height(Length::Fill)
            .into()
    }

    pub fn update(&mut self, message: Message) -> Task<app::Message> {
        let mut tasks = vec![];
        match message {
            Message::SetClient(mastodon) => self.mastodon = mastodon,
            Message::LoadMore(load) => {
                if !self.loading && load {
                    self.loading = true;
                    self.skip += 20;
                }
            }
            Message::AppendStatus(status) => {
                self.loading = false;
                self.statuses.push_back(status.id.clone());
                tasks.push(cosmic::task::message(app::Message::CacheStatus(
                    status.clone(),
                )));

                tasks.push(cosmic::task::message(app::Message::Fetch(
                    utils::extract_status_images(&status),
                )));
            }
            Message::PrependStatus(status) => {
                self.loading = false;
                self.statuses.push_front(status.id.clone());
                tasks.push(cosmic::task::message(app::Message::CacheStatus(status)));
            }
            Message::DeleteStatus(id) => self
                .statuses
                .retain(|status_id| *status_id.to_string() != id),
            Message::Status(message) => tasks.push(widgets::status::update(message)),
        }
        Task::batch(tasks)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        if self.is_authenticated()
            && (self.statuses.is_empty() || self.statuses.len() != self.skip + 20)
        {
            Subscription::batch(vec![crate::subscriptions::home::user_timeline(
                self.mastodon.clone(),
                self.skip,
            )])
        } else {
            Subscription::none()
        }
    }
}
