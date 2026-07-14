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
            filter: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.mastodon.is_authenticated()
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

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

        let notifications: Vec<Element<_>> = self
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
                self.loading = false;
                self.max_id = Some(notification.id.clone());
                self.notifications.push_back(notification.id.clone());
                tasks.push(cosmic::task::message(app::Message::CacheNotification(
                    notification.clone(),
                )));

                tasks.push(cosmic::task::message(app::Message::Fetch(
                    cache::extract_notification_images(&notification),
                )));
            }
            Message::PrependNotification(notification) => {
                self.notifications.push_front(notification.id.clone());
                tasks.push(cosmic::task::message(app::Message::CacheNotification(
                    notification,
                )));
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
        if self.is_authenticated() && (self.notifications.is_empty() || self.loading) {
            return Subscription::batch(vec![fetch::timeline(
                self.mastodon.clone(),
                self.max_id.clone(),
            )]);
        }

        Subscription::none()
    }
}
