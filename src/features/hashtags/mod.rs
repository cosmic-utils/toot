//! Followed hashtags: browse followed tags and view a selected tag's timeline.

use cosmic::{app::Task, iced::Subscription, widget, Apply, Element};
use megalodon::entities::Tag;

use crate::{
    app,
    cache::Cache,
    client::Client,
    features::timeline::{self, Timeline, TimelineKind},
};

pub struct Hashtags {
    mastodon: Client,
    followed: Vec<Tag>,
    loaded: bool,
    selected: Option<Timeline>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Client),
    SetFollowed(Vec<Tag>),
    Select(String),
    Timeline(timeline::Message),
}

impl Hashtags {
    pub fn new(mastodon: Client) -> Self {
        Self {
            mastodon,
            followed: Vec::new(),
            loaded: false,
            selected: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.mastodon.is_authenticated()
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        let tags: Vec<Element<_>> = self
            .followed
            .iter()
            .map(|tag| {
                widget::button::suggested(format!("#{}", tag.name))
                    .on_press(Message::Select(tag.name.clone()))
                    .into()
            })
            .collect();

        let content = self
            .selected
            .as_ref()
            .map(|timeline| timeline.view(cache).map(Message::Timeline));

        widget::column![
            widget::scrollable(widget::row(tags).spacing(spacing.space_xs))
                .direction(cosmic::iced::widget::scrollable::Direction::Horizontal(
                    Default::default()
                )),
            content,
        ]
        .spacing(spacing.space_xs)
        .apply(widget::container)
        .max_width(700)
        .into()
    }

    pub fn update(&mut self, message: Message) -> Task<app::Message> {
        match message {
            Message::SetClient(mastodon) => {
                self.mastodon = mastodon.clone();
                if let Some(timeline) = &mut self.selected {
                    return timeline.update(timeline::Message::SetClient(mastodon));
                }
            }
            Message::SetFollowed(tags) => {
                self.followed = tags;
                self.loaded = true;
            }
            Message::Select(name) => {
                self.selected = Some(Timeline::new(self.mastodon.clone(), TimelineKind::Tag(name)));
            }
            Message::Timeline(message) => {
                if let Some(timeline) = &mut self.selected {
                    return timeline.update(message);
                }
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![];
        if self.is_authenticated() && !self.loaded {
            subscriptions.push(fetch_followed_tags(self.mastodon.clone()));
        }
        if let Some(timeline) = &self.selected {
            subscriptions.push(timeline.subscription().map(Message::Timeline));
        }
        Subscription::batch(subscriptions)
    }
}

fn fetch_followed_tags(mastodon: Client) -> Subscription<Message> {
    Subscription::run_with(mastodon, |mastodon| {
        let mastodon = mastodon.clone();
        cosmic::iced::stream::channel(
            1,
            move |mut output: futures_channel::mpsc::Sender<Message>| async move {
                use futures_util::SinkExt;
                match mastodon.get_followed_tags().await {
                    Ok(response) => {
                        if let Err(err) = output.send(Message::SetFollowed(response.json)).await {
                            tracing::warn!("failed to send followed tags: {}", err);
                        }
                    }
                    Err(err) => tracing::warn!("failed to get followed tags: {}", err),
                }
                std::future::pending().await
            },
        )
    })
}
