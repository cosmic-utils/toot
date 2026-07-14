//! Notifications feed: fetching, caching, filtering, and rendering the
//! user's notifications, plus follow-request accept/reject and clear-all.

pub mod fetch;
pub mod view;

use std::collections::VecDeque;

use cosmic::{
    app::Task,
    iced::widget::scrollable::{Direction, Scrollbar},
    iced::{Length, Subscription},
    widget, Apply, Element,
};
use megalodon::entities::{notification::NotificationType, Notification};

use crate::{app, cache, cache::Cache, client::Client, features::status};

#[derive(Debug, Clone)]
pub struct Notifications {
    pub mastodon: Client,
    notifications: VecDeque<String>,
    max_id: Option<String>,
    loading: bool,
    has_loaded: bool,
    filter: Option<NotificationType>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Client),
    AppendNotification(Notification),
    PrependNotification(Notification),
    Notification(view::Message),
    LoadMore(bool),
    SetFilter(Option<NotificationType>),
    ClearAll,
    /// A fetch's result stream has ended (successfully, even if empty).
    LoadComplete,
}

const FILTERS: [Option<NotificationType>; 5] = [
    None,
    Some(NotificationType::Mention),
    Some(NotificationType::Reblog),
    Some(NotificationType::Favourite),
    Some(NotificationType::Follow),
];

fn filter_label(filter: &Option<NotificationType>) -> &'static str {
    match filter {
        None => "All",
        Some(NotificationType::Mention) => "Mentions",
        Some(NotificationType::Reblog) => "Boosts",
        Some(NotificationType::Favourite) => "Favorites",
        Some(NotificationType::Follow) => "Follows",
        Some(_) => "Other",
    }
}

impl Notifications {
    pub fn new(mastodon: Client) -> Self {
        Self {
            mastodon,
            notifications: VecDeque::new(),
            max_id: None,
            loading: false,
            has_loaded: false,
            filter: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.mastodon.is_authenticated()
    }

    /// Switch to a different account's client and drop this feed's current
    /// content. Call [`Notifications::load_cached`] afterward to repopulate
    /// from disk.
    pub fn reset(&mut self, mastodon: Client) {
        self.mastodon = mastodon;
        self.notifications.clear();
        self.max_id = None;
        self.loading = false;
        self.has_loaded = false;
    }

    /// Whether the initial fetch is still in flight (no content yet, and no
    /// fetch has completed to confirm the feed is genuinely empty).
    pub fn is_initial_loading(&self) -> bool {
        self.notifications.is_empty() && !self.has_loaded
    }

    /// Load the last-saved notification snapshot from disk (if any) so the
    /// view has something to render immediately, before the network fetch lands.
    pub fn load_cached(&mut self) -> Task<app::Message> {
        let cached: Vec<Notification> =
            crate::persistence::load_snapshot(&self.mastodon.base_url, "notifications");
        let mut tasks = vec![];
        for notification in cached {
            if !self.notifications.contains(&notification.id) {
                self.notifications.push_back(notification.id.clone());
            }
            tasks.push(cosmic::task::message(app::Message::Fetch(
                cache::extract_notification_images(&notification),
            )));
            tasks.push(cosmic::task::message(app::Message::CacheNotification(
                notification,
            )));
        }
        Task::batch(tasks)
    }

    /// Persist the currently-cached notifications to disk.
    pub fn save_cached(&self, cache: &Cache) {
        let notifications: Vec<Notification> = self
            .notifications
            .iter()
            .filter_map(|id| cache.notifications.get(id).cloned())
            .collect();
        crate::persistence::save_notification_snapshot(&self.mastodon.base_url, &notifications);
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

        if self.is_initial_loading() {
            return widget::container(widget::indeterminate_circular().size(40.0))
                .center(Length::Fill)
                .into();
        }

        let labels: Vec<&str> = FILTERS.iter().map(filter_label).collect();
        let selected = FILTERS.iter().position(|f| f == &self.filter);
        let toolbar = widget::row![
            widget::dropdown(labels, selected, |index| {
                Message::SetFilter(FILTERS[index].clone())
            }),
            widget::space::horizontal(),
            widget::button::standard("Clear all").on_press(Message::ClearAll),
        ]
        .spacing(spacing.space_xs)
        .padding(spacing.space_xs);

        let mut notifications: Vec<Element<_>> = self
            .notifications
            .iter()
            .filter_map(|id| cache.notifications.get(id))
            .filter(|notification| {
                self.filter
                    .as_ref()
                    .is_none_or(|filter| notification.r#type == *filter)
            })
            .map(|notification| view::notification(notification, cache).map(Message::Notification))
            .collect();

        if notifications.is_empty() {
            return widget::column![
                toolbar,
                widget::container(widget::text("Nothing here yet")).center(Length::Fill)
            ]
            .apply(widget::container)
            .max_width(700)
            .height(Length::Fill)
            .into();
        }

        if self.loading {
            notifications.push(
                widget::container(widget::indeterminate_circular().size(24.0))
                    .center_x(Length::Fill)
                    .padding(spacing.space_s)
                    .into(),
            );
        }

        widget::column![
            toolbar,
            widget::scrollable(widget::settings::section().extend(notifications))
                .direction(Direction::Vertical(
                    Scrollbar::default().spacing(spacing.space_xxs),
                ))
                .on_scroll(|viewport| {
                    Message::LoadMore(!self.loading && viewport.relative_offset().y == 1.0)
                })
        ]
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
            Message::AppendNotification(notification) => {
                self.max_id = Some(notification.id.clone());
                if !self.notifications.contains(&notification.id) {
                    self.notifications.push_back(notification.id.clone());
                }
                tasks.push(cosmic::task::message(app::Message::CacheNotification(
                    notification.clone(),
                )));

                tasks.push(cosmic::task::message(app::Message::Fetch(
                    cache::extract_notification_images(&notification),
                )));
            }
            Message::PrependNotification(notification) => {
                if !self.notifications.contains(&notification.id) {
                    self.notifications.push_front(notification.id.clone());
                }
                tasks.push(cosmic::task::message(app::Message::CacheNotification(
                    notification,
                )));
            }
            Message::LoadComplete => {
                self.loading = false;
                self.has_loaded = true;
            }
            Message::SetFilter(filter) => self.filter = filter,
            Message::ClearAll => {
                self.notifications.clear();
                let mastodon = self.mastodon.clone();
                tasks.push(cosmic::task::future(async move {
                    match mastodon.dismiss_notifications().await {
                        Ok(_) => app::Message::None,
                        Err(err) => {
                            app::Message::Error(format!("Couldn't clear notifications: {err}"))
                        }
                    }
                }));
            }
            Message::Notification(message) => match message {
                view::Message::Status(message) => tasks.push(status::update(message)),
                view::Message::AcceptFollowRequest(notification_id, account_id) => {
                    self.notifications.retain(|id| *id != notification_id);
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        match mastodon.accept_follow_request(account_id).await {
                            Ok(response) => app::Message::CacheRelationship(response.json),
                            Err(err) => {
                                app::Message::Error(format!("Couldn't accept follow: {err}"))
                            }
                        }
                    }));
                }
                view::Message::RejectFollowRequest(notification_id, account_id) => {
                    self.notifications.retain(|id| *id != notification_id);
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        match mastodon.reject_follow_request(account_id).await {
                            Ok(response) => app::Message::CacheRelationship(response.json),
                            Err(err) => {
                                app::Message::Error(format!("Couldn't reject follow: {err}"))
                            }
                        }
                    }));
                }
            },
        }
        Task::batch(tasks)
    }

    pub fn subscription(&self) -> Subscription<Message> {
        // `!self.has_loaded`, not `self.notifications.is_empty()`: a
        // cache-preloaded feed already has notifications to show, but still
        // needs its first real network fetch to refresh in the background.
        if self.is_authenticated() && (!self.has_loaded || self.loading) {
            return Subscription::batch(vec![fetch::timeline(
                self.mastodon.clone(),
                self.max_id.clone(),
            )]);
        }

        Subscription::none()
    }
}
