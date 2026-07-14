//! User lists: browse the account's lists and view a selected list's timeline.

use cosmic::{
    app::Task,
    iced::{Alignment, Length, Subscription},
    widget, Apply, Element,
};
use megalodon::entities::List;

use crate::{
    app,
    cache::Cache,
    client::Client,
    features::timeline::{self, Timeline, TimelineKind},
};

pub struct Lists {
    mastodon: Client,
    lists: Vec<List>,
    loaded: bool,
    selected: Option<Timeline>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Client),
    SetLists(Vec<List>),
    Select(String),
    Timeline(timeline::Message),
}

impl Lists {
    pub fn new(mastodon: Client) -> Self {
        Self {
            mastodon,
            lists: Vec::new(),
            loaded: false,
            selected: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.mastodon.is_authenticated()
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;
        let lists: Vec<Element<_>> = self
            .lists
            .iter()
            .map(|list| {
                widget::button::suggested(list.title.clone())
                    .on_press(Message::Select(list.id.clone()))
                    .into()
            })
            .collect();

        let content = self
            .selected
            .as_ref()
            .map(|timeline| timeline.view(cache).map(Message::Timeline));

        widget::column![
            widget::scrollable(widget::row(lists).spacing(spacing.space_xs))
                .direction(cosmic::iced::widget::scrollable::Direction::Horizontal(
                    Default::default()
                )),
            content,
        ]
        .spacing(spacing.space_xs)
        .width(Length::Fill)
        .height(Length::Fill)
        .apply(widget::container)
        .max_width(700)
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Alignment::Start)
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
            Message::SetLists(lists) => {
                self.lists = lists;
                self.loaded = true;
            }
            Message::Select(id) => {
                let mut timeline = Timeline::new(self.mastodon.clone(), TimelineKind::List(id));
                let task = timeline.load_cached();
                self.selected = Some(timeline);
                return task;
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
            subscriptions.push(fetch_lists(self.mastodon.clone()));
        }
        if let Some(timeline) = &self.selected {
            subscriptions.push(timeline.subscription().map(Message::Timeline));
        }
        Subscription::batch(subscriptions)
    }
}

fn fetch_lists(mastodon: Client) -> Subscription<Message> {
    Subscription::run_with(mastodon, |mastodon| {
        let mastodon = mastodon.clone();
        cosmic::iced::stream::channel(
            1,
            move |mut output: futures_channel::mpsc::Sender<Message>| async move {
                use futures_util::SinkExt;
                match mastodon.get_lists().await {
                    Ok(response) => {
                        if let Err(err) = output.send(Message::SetLists(response.json)).await {
                            tracing::warn!("failed to send lists: {}", err);
                        }
                    }
                    Err(err) => tracing::warn!("failed to get lists: {}", err),
                }
                std::future::pending().await
            },
        )
    })
}
