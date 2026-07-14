//! Followed hashtags: browse followed tags, follow/unfollow them, and view a
//! selected tag's timeline.

use std::collections::HashSet;

use cosmic::{
    app::Task,
    iced::{Alignment, Length, Subscription},
    widget, Apply, Element,
};
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
    /// Tags the user has unfollowed during this visit to the page. Kept
    /// visible (with an updated button) until the page is left and revisited,
    /// at which point the followed-tags list is refetched and they drop out.
    unfollowed: HashSet<String>,
    filter: String,
    selected: Option<Timeline>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Client),
    /// The page became active again; refresh the followed list from the
    /// server so tags unfollowed earlier finally disappear.
    Refresh,
    SetFollowed(Vec<Tag>),
    FilterChanged(String),
    Select(String),
    /// Deselect the current tag, returning to the followed-tags list.
    Deselect,
    /// Toggle following a tag: (name, currently following).
    ToggleFollow(String, bool),
    FollowResult(String, bool),
    Timeline(timeline::Message),
}

impl Hashtags {
    pub fn new(mastodon: Client) -> Self {
        Self {
            mastodon,
            followed: Vec::new(),
            loaded: false,
            unfollowed: HashSet::new(),
            filter: String::new(),
            selected: None,
        }
    }

    pub fn is_authenticated(&self) -> bool {
        self.mastodon.is_authenticated()
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

        // Once a tag is selected, collapse the list and show only its feed
        // (with a way back), rather than showing both at once.
        if let Some(timeline) = &self.selected {
            return widget::column![
                widget::button::standard("← Back to hashtags").on_press(Message::Deselect),
                timeline.view(cache).map(Message::Timeline),
            ]
            .spacing(spacing.space_xs)
            .width(Length::Fill)
            .height(Length::Fill)
            .apply(widget::container)
            .max_width(700)
            .width(Length::Fill)
            .height(Length::Fill)
            .align_x(Alignment::Start)
            .into();
        }

        let filter_input = widget::text_input("Filter hashtags", &self.filter)
            .on_input(Message::FilterChanged);

        let filter = self.filter.to_lowercase();
        let section = self
            .followed
            .iter()
            .filter(|tag| filter.is_empty() || tag.name.to_lowercase().contains(&filter))
            .fold(
                widget::settings::section().title("Followed hashtags"),
                |section, tag| {
                    let following = !self.unfollowed.contains(&tag.name);
                    section.add(widget::settings::item_row(vec![
                        widget::button::link(format!("#{}", tag.name))
                            .on_press(Message::Select(tag.name.clone()))
                            .into(),
                        widget::space::horizontal().into(),
                        widget::button::standard(if following { "Unfollow" } else { "Follow" })
                            .on_press(Message::ToggleFollow(tag.name.clone(), following))
                            .into(),
                    ]))
                },
            );

        widget::column![
            filter_input,
            widget::scrollable(section).width(Length::Fill).height(Length::Fill),
        ]
        .spacing(spacing.space_xs)
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
            Message::Refresh => {
                self.loaded = false;
                self.unfollowed.clear();
            }
            Message::SetFollowed(tags) => {
                self.followed = tags;
                self.loaded = true;
            }
            Message::FilterChanged(filter) => self.filter = filter,
            Message::Select(name) => {
                let mut timeline = Timeline::new(self.mastodon.clone(), TimelineKind::Tag(name));
                let task = timeline.load_cached();
                self.selected = Some(timeline);
                return task;
            }
            Message::Deselect => self.selected = None,
            Message::ToggleFollow(name, following) => {
                let mastodon = self.mastodon.clone();
                return cosmic::task::future(async move {
                    let result = if following {
                        mastodon.unfollow_tag(name.clone()).await
                    } else {
                        mastodon.follow_tag(name.clone()).await
                    };
                    match result {
                        Ok(response) => app::Message::Hashtags(Message::FollowResult(
                            name,
                            response.json.following.unwrap_or(!following),
                        )),
                        Err(err) => app::Message::Error(format!("Couldn't update hashtag follow: {err}")),
                    }
                });
            }
            Message::FollowResult(name, following) => {
                if following {
                    self.unfollowed.remove(&name);
                } else {
                    self.unfollowed.insert(name);
                }
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
