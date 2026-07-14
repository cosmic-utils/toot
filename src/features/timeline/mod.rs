//! Feed-shaped timelines: Home, Explore (public), Local, and Federated.
//!
//! All four share the same fetch/paginate/render shape, so they're modeled as
//! one [`Timeline`] parameterized by [`TimelineKind`] instead of four
//! near-identical structs.

pub mod fetch;

use std::collections::VecDeque;

use cosmic::{
    app::Task,
    iced::widget::scrollable::{Direction, Scrollbar},
    iced::{Length, Subscription},
    widget, Apply, Element,
};
use megalodon::entities::Status;

use crate::{
    app,
    cache::{self, Cache},
    client::Client,
    features::status::{self, StatusOptions},
};

/// Which Mastodon timeline a [`Timeline`] instance renders.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TimelineKind {
    /// The authenticated user's home feed.
    Home,
    /// The public (federated) timeline, unfiltered.
    Public,
    /// The local instance's public timeline.
    Local,
    /// The public timeline, filtered down to posts from remote instances.
    Federated,
    /// Statuses the user has favourited.
    Favorites,
    /// Statuses the user has bookmarked.
    Bookmarks,
    /// A hashtag's timeline.
    Tag(String),
    /// A user list's timeline.
    List(String),
}

/// State for a single feed-shaped timeline.
#[derive(Debug, Clone)]
pub struct Timeline {
    pub mastodon: Client,
    kind: TimelineKind,
    statuses: VecDeque<String>,
    max_id: Option<String>,
    loading: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Client),
    AppendStatus(Status),
    PrependStatus(Status),
    DeleteStatus(String),
    Status(status::Message),
    LoadMore(bool),
}

impl Timeline {
    pub fn new(mastodon: Client, kind: TimelineKind) -> Self {
        Self {
            mastodon,
            kind,
            statuses: VecDeque::new(),
            max_id: None,
            loading: false,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.mastodon.is_authenticated()
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        let statuses: Vec<Element<_>> = self
            .statuses
            .iter()
            .filter_map(|id| cache.statuses.get(id))
            .map(|status| status::status(status, StatusOptions::all(), cache).map(Message::Status))
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
                }
            }
            Message::AppendStatus(status) => {
                self.loading = false;
                self.max_id = Some(status.id.clone());
                self.statuses.push_back(status.id.clone());
                tasks.push(cosmic::task::message(app::Message::CacheStatus(
                    status.clone(),
                )));

                tasks.push(cosmic::task::message(app::Message::Fetch(
                    cache::extract_status_images(&status),
                )));
            }
            Message::PrependStatus(status) => {
                self.statuses.push_front(status.id.clone());
                tasks.push(cosmic::task::message(app::Message::CacheStatus(status)));
            }
            Message::DeleteStatus(id) => self.statuses.retain(|status_id| *status_id != id),
            Message::Status(message) => tasks.push(status::update(message)),
        }
        Task::batch(tasks)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let requires_auth = matches!(
            self.kind,
            TimelineKind::Home | TimelineKind::Favorites | TimelineKind::Bookmarks
        );
        if requires_auth && !self.is_authenticated() {
            return Subscription::none();
        }

        if self.statuses.is_empty() || self.loading {
            Subscription::batch(vec![fetch::timeline(
                self.mastodon.clone(),
                self.kind.clone(),
                self.max_id.clone(),
            )])
        } else {
            Subscription::none()
        }
    }
}
