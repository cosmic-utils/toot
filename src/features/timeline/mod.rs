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

impl TimelineKind {
    /// A filesystem-safe identifier used as the disk cache filename stem.
    pub fn slug(&self) -> String {
        match self {
            TimelineKind::Home => "home".to_string(),
            TimelineKind::Public => "public".to_string(),
            TimelineKind::Local => "local".to_string(),
            TimelineKind::Federated => "federated".to_string(),
            TimelineKind::Favorites => "favorites".to_string(),
            TimelineKind::Bookmarks => "bookmarks".to_string(),
            TimelineKind::Tag(name) => format!("tag-{name}"),
            TimelineKind::List(id) => format!("list-{id}"),
        }
    }
}

/// State for a single feed-shaped timeline.
#[derive(Debug, Clone)]
pub struct Timeline {
    pub mastodon: Client,
    kind: TimelineKind,
    statuses: VecDeque<String>,
    max_id: Option<String>,
    /// Whether a page-load (initial or "load more") fetch is in flight.
    loading: bool,
    /// Whether at least one fetch has completed, so an empty `statuses` can
    /// be told apart from "hasn't tried yet" (still loading).
    has_loaded: bool,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Client),
    AppendStatus(Status),
    PrependStatus(Status),
    DeleteStatus(String),
    Status(status::Message),
    LoadMore(bool),
    /// A fetch's result stream has ended (successfully, even if empty).
    LoadComplete,
}

impl Timeline {
    pub fn new(mastodon: Client, kind: TimelineKind) -> Self {
        Self {
            mastodon,
            kind,
            statuses: VecDeque::new(),
            max_id: None,
            loading: false,
            has_loaded: false,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.mastodon.is_authenticated()
    }

    /// Switch to a different account's client and drop this feed's current
    /// content, so a stale account's posts don't linger after switching.
    /// Call [`Timeline::load_cached`] afterward to repopulate from disk.
    pub fn reset(&mut self, mastodon: Client) {
        self.mastodon = mastodon;
        self.statuses.clear();
        self.max_id = None;
        self.loading = false;
        self.has_loaded = false;
    }

    /// Whether the initial fetch is still in flight (no content yet, and no
    /// fetch has completed to confirm the feed is genuinely empty).
    pub fn is_initial_loading(&self) -> bool {
        self.statuses.is_empty() && !self.has_loaded
    }

    /// Load this feed's last-saved snapshot from disk (if any) so the view
    /// has something to render immediately, before the network fetch lands.
    pub fn load_cached(&mut self) -> Task<app::Message> {
        let cached: Vec<Status> =
            crate::persistence::load_snapshot(&self.mastodon.base_url, &self.kind.slug());
        let mut tasks = vec![];
        for status in cached {
            if !self.statuses.contains(&status.id) {
                self.statuses.push_back(status.id.clone());
            }
            tasks.push(cosmic::task::message(app::Message::Fetch(
                cache::extract_status_images(&status),
            )));
            tasks.push(cosmic::task::message(app::Message::CacheStatus(status)));
        }
        Task::batch(tasks)
    }

    /// Persist the currently-cached statuses for this feed to disk.
    pub fn save_cached(&self, cache: &Cache) {
        let statuses: Vec<Status> = self
            .statuses
            .iter()
            .filter_map(|id| cache.statuses.get(id).cloned())
            .collect();
        crate::persistence::save_status_snapshot(
            &self.mastodon.base_url,
            &self.kind.slug(),
            &statuses,
        );
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

        if self.is_initial_loading() {
            return widget::container(widget::indeterminate_circular().size(40.0))
                .center(Length::Fill)
                .into();
        }

        let mut statuses: Vec<Element<_>> = self
            .statuses
            .iter()
            .filter_map(|id| cache.statuses.get(id))
            .filter(|status| cache.is_visible(status))
            .map(|status| status::status(status, StatusOptions::all(), cache).map(Message::Status))
            .collect();

        if statuses.is_empty() {
            return widget::container(widget::text("Nothing here yet"))
                .center(Length::Fill)
                .into();
        }

        if self.loading {
            statuses.push(
                widget::container(widget::indeterminate_circular().size(24.0))
                    .center_x(Length::Fill)
                    .padding(spacing.space_s)
                    .into(),
            );
        }

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
                self.max_id = Some(status.id.clone());
                if !self.statuses.contains(&status.id) {
                    self.statuses.push_back(status.id.clone());
                }
                tasks.push(cosmic::task::message(app::Message::CacheStatus(
                    status.clone(),
                )));

                tasks.push(cosmic::task::message(app::Message::Fetch(
                    cache::extract_status_images(&status),
                )));
            }
            Message::PrependStatus(status) => {
                if !self.statuses.contains(&status.id) {
                    self.statuses.push_front(status.id.clone());
                }
                tasks.push(cosmic::task::message(app::Message::CacheStatus(status)));
            }
            Message::DeleteStatus(id) => self.statuses.retain(|status_id| *status_id != id),
            Message::LoadComplete => {
                self.loading = false;
                self.has_loaded = true;
            }
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

        // `!self.has_loaded`, not `self.statuses.is_empty()`: a cache-preloaded
        // feed already has statuses to show, but still needs its first real
        // network fetch to refresh in the background — checking emptiness
        // here would skip that fetch entirely until the user scrolls.
        if !self.has_loaded || self.loading {
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
