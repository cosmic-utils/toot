//! Search: accounts, hashtags, and statuses matching a query, plus a
//! trending-hashtags view while idle.

use std::collections::HashMap;

use cosmic::{
    app::Task,
    iced::{Alignment, Length, Subscription},
    widget::{self, segmented_button},
    Apply, Element,
};
use megalodon::{
    entities::{Account, Results, Tag},
    megalodon::SearchInputOptions,
};

use crate::{
    app,
    cache::{self, Cache},
    client::Client,
    features::{
        status::{self, StatusOptions},
        timeline::{self, Timeline, TimelineKind},
    },
};

/// Which kind of result the segmented control is showing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ResultKind {
    Accounts,
    Hashtags,
    Statuses,
}

pub struct Search {
    mastodon: Client,
    query: String,
    results: Option<Results>,
    searching: bool,
    result_tabs: segmented_button::SingleSelectModel,
    trending: Vec<Tag>,
    trending_loaded: bool,
    /// Follow-state overrides for hashtags toggled this session, keyed by
    /// name — used for both trending and search-result tags.
    tag_follow_overrides: HashMap<String, bool>,
    tag_timeline: Option<Timeline>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Client),
    QueryChanged(String),
    Submit,
    /// Clear the query and any results, returning to the idle/trending view.
    Clear,
    SetResults(Results),
    SearchFailed(String),
    SelectResultKind(segmented_button::Entity),
    OpenAccount(Account),
    ToggleFollowAccount(String, bool),
    SelectTag(String),
    /// Deselect the current tag, returning to the search results/idle view.
    DeselectTag,
    ToggleFollowTag(String, bool),
    TagFollowResult(String, bool),
    SetTrending(Vec<Tag>),
    Status(status::Message),
    Timeline(timeline::Message),
}

impl Search {
    pub fn new(mastodon: Client) -> Self {
        let mut result_tabs = segmented_button::SingleSelectModel::default();
        result_tabs.insert().text("Users").data(ResultKind::Accounts).activate();
        result_tabs.insert().text("Hashtags").data(ResultKind::Hashtags);
        result_tabs.insert().text("Posts").data(ResultKind::Statuses);

        Self {
            mastodon,
            query: String::new(),
            results: None,
            searching: false,
            result_tabs,
            trending: Vec::new(),
            trending_loaded: false,
            tag_follow_overrides: HashMap::new(),
            tag_timeline: None,
        }
    }

    fn is_following_tag(&self, tag: &Tag) -> bool {
        self.tag_follow_overrides
            .get(&tag.name)
            .copied()
            .unwrap_or_else(|| tag.following.unwrap_or(false))
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

        // Once a tag is selected, collapse everything else and show only its
        // feed (with a way back), rather than piling it below the results.
        if let Some(timeline) = &self.tag_timeline {
            return widget::column![
                widget::button::standard("← Back to search").on_press(Message::DeselectTag),
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

        let has_query_or_results = !self.query.is_empty() || self.results.is_some();
        let input = widget::row![
            widget::text_input("Search accounts, hashtags, statuses", &self.query)
                .on_input(Message::QueryChanged)
                .on_submit(|_| Message::Submit),
        ]
        .push_maybe(has_query_or_results.then(|| -> Element<'_, Message> {
            widget::button::standard("Clear")
                .on_press(Message::Clear)
                .into()
        }))
        .spacing(spacing.space_xs);

        let searching: Option<Element<_>> = self.searching.then(|| {
            widget::container(widget::indeterminate_circular().size(24.0))
                .center_x(Length::Fill)
                .padding(spacing.space_s)
                .into()
        });

        let idle: Option<Element<_>> = (self.results.is_none() && !self.searching).then(|| {
            self.trending
                .iter()
                .fold(
                    widget::settings::section().title("Trending hashtags"),
                    |section, tag| section.add(tag_row(tag, self.is_following_tag(tag))),
                )
                .into()
        });

        let tabs: Option<Element<_>> = self.results.is_some().then(|| {
            widget::segmented_control::horizontal(&self.result_tabs)
                .on_activate(Message::SelectResultKind)
                .into()
        });

        let results_content: Option<Element<_>> = self.results.as_ref().map(|results| {
            match self.result_tabs.active_data::<ResultKind>() {
                Some(ResultKind::Accounts) => results
                    .accounts
                    .iter()
                    .fold(widget::settings::section(), |section, account| {
                        section.add(account_row(account, cache))
                    })
                    .into(),
                Some(ResultKind::Hashtags) => results
                    .hashtags
                    .iter()
                    .fold(widget::settings::section(), |section, tag| {
                        section.add(tag_row(tag, self.is_following_tag(tag)))
                    })
                    .into(),
                Some(ResultKind::Statuses) | None => widget::column(
                    results
                        .statuses
                        .iter()
                        .map(|status| status::status(status, StatusOptions::all(), cache).map(Message::Status))
                        .collect::<Vec<_>>(),
                )
                .spacing(spacing.space_xs)
                .into(),
            }
        });

        widget::column![
            input,
            widget::scrollable(
                widget::column![searching, idle, tabs, results_content].spacing(spacing.space_s),
            )
            .width(Length::Fill)
            .height(Length::Fill),
        ]
        .spacing(spacing.space_s)
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
                if let Some(timeline) = &mut self.tag_timeline {
                    return timeline.update(timeline::Message::SetClient(mastodon));
                }
            }
            Message::QueryChanged(query) => self.query = query,
            Message::Submit => {
                let query = self.query.trim().to_string();
                if !query.is_empty() {
                    self.searching = true;
                    let mastodon = self.mastodon.clone();
                    return cosmic::task::future(async move {
                        match mastodon.search(query, Some(&SearchInputOptions::default())).await {
                            Ok(response) => {
                                app::Message::Search(Message::SetResults(response.json))
                            }
                            Err(err) => app::Message::Search(Message::SearchFailed(err.to_string())),
                        }
                    });
                }
            }
            Message::Clear => {
                self.query.clear();
                self.results = None;
                self.tag_timeline = None;
            }
            Message::SetResults(results) => {
                self.searching = false;
                let account_ids: Vec<String> =
                    results.accounts.iter().map(|account| account.id.clone()).collect();

                let mut image_urls: Vec<String> = results
                    .accounts
                    .iter()
                    .map(|account| account.avatar.clone())
                    .collect();
                for status in &results.statuses {
                    image_urls.extend(cache::extract_status_images(status));
                }

                self.results = Some(results);

                let mut tasks = vec![cosmic::task::message(app::Message::Fetch(image_urls))];

                if !account_ids.is_empty() && self.mastodon.is_authenticated() {
                    let mastodon = self.mastodon.clone();
                    tasks.push(cosmic::task::future(async move {
                        match mastodon.get_relationships(account_ids).await {
                            Ok(response) => app::Message::CacheRelationships(response.json),
                            Err(err) => {
                                app::Message::Error(format!("Couldn't load relationships: {err}"))
                            }
                        }
                    }));
                }
                return Task::batch(tasks);
            }
            Message::SearchFailed(err) => {
                self.searching = false;
                return cosmic::task::message(app::Message::Error(format!("Search failed: {err}")));
            }
            Message::SelectResultKind(entity) => self.result_tabs.activate(entity),
            Message::OpenAccount(account) => {
                return cosmic::task::message(app::Message::ToggleContextPage(
                    app::ContextPage::Account(account),
                ));
            }
            Message::ToggleFollowAccount(id, following) => {
                let mastodon = self.mastodon.clone();
                return cosmic::task::future(async move {
                    let result = if following {
                        mastodon.unfollow_account(id).await
                    } else {
                        mastodon.follow_account(id, None).await
                    };
                    match result {
                        Ok(response) => app::Message::CacheRelationship(response.json),
                        Err(err) => app::Message::Error(format!("Couldn't update follow: {err}")),
                    }
                });
            }
            Message::SelectTag(name) => {
                let mut timeline = Timeline::new(self.mastodon.clone(), TimelineKind::Tag(name));
                let task = timeline.load_cached();
                self.tag_timeline = Some(timeline);
                return task;
            }
            Message::DeselectTag => self.tag_timeline = None,
            Message::ToggleFollowTag(name, following) => {
                let mastodon = self.mastodon.clone();
                return cosmic::task::future(async move {
                    let result = if following {
                        mastodon.unfollow_tag(name.clone()).await
                    } else {
                        mastodon.follow_tag(name.clone()).await
                    };
                    match result {
                        Ok(response) => app::Message::Search(Message::TagFollowResult(
                            name,
                            response.json.following.unwrap_or(!following),
                        )),
                        Err(err) => {
                            app::Message::Error(format!("Couldn't update hashtag follow: {err}"))
                        }
                    }
                });
            }
            Message::TagFollowResult(name, following) => {
                self.tag_follow_overrides.insert(name, following);
            }
            Message::SetTrending(tags) => {
                self.trending = tags;
                self.trending_loaded = true;
            }
            Message::Status(message) => return status::update(message),
            Message::Timeline(message) => {
                if let Some(timeline) = &mut self.tag_timeline {
                    return timeline.update(message);
                }
            }
        }
        Task::none()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let mut subscriptions = vec![];
        if !self.trending_loaded {
            subscriptions.push(fetch_trending(self.mastodon.clone()));
        }
        if let Some(timeline) = &self.tag_timeline {
            subscriptions.push(timeline.subscription().map(Message::Timeline));
        }
        Subscription::batch(subscriptions)
    }
}

fn account_row<'a>(account: &'a Account, cache: &'a Cache) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    let following = cache
        .relationships
        .get(&account.id)
        .is_some_and(|relationship| relationship.following);

    widget::settings::item_row(vec![
        widget::mouse_area(
            widget::row![
                cache
                    .handles
                    .get(&account.avatar)
                    .map(|handle| widget::image(handle).width(32).height(32))
                    .unwrap_or(crate::cache::fallback_avatar().width(32).height(32)),
                widget::column![
                    widget::text(account.display_name.clone()),
                    widget::text::caption(format!("@{}", account.acct)),
                ],
            ]
            .spacing(spacing.space_xs)
            .align_y(Alignment::Center),
        )
        .on_press(Message::OpenAccount(account.clone()))
        .interaction(cosmic::iced::mouse::Interaction::Pointer)
        .into(),
        widget::space::horizontal().into(),
        widget::button::standard(if following { "Unfollow" } else { "Follow" })
            .on_press(Message::ToggleFollowAccount(account.id.clone(), following))
            .into(),
    ])
    .into()
}

fn tag_row(tag: &Tag, following: bool) -> Element<'_, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    widget::settings::item_row(vec![
        widget::button::link(format!("#{}", tag.name))
            .on_press(Message::SelectTag(tag.name.clone()))
            .into(),
        widget::space::horizontal().into(),
        widget::button::standard(if following { "Unfollow" } else { "Follow" })
            .on_press(Message::ToggleFollowTag(tag.name.clone(), following))
            .into(),
    ])
    .spacing(spacing.space_xs)
    .into()
}

fn fetch_trending(mastodon: Client) -> Subscription<Message> {
    Subscription::run_with(mastodon, |mastodon| {
        let mastodon = mastodon.clone();
        cosmic::iced::stream::channel(
            1,
            move |mut output: futures_channel::mpsc::Sender<Message>| async move {
                use futures_util::SinkExt;
                match mastodon.get_instance_trends(Some(10)).await {
                    Ok(response) => {
                        if let Err(err) = output.send(Message::SetTrending(response.json)).await {
                            tracing::warn!("failed to send trending tags: {}", err);
                        }
                    }
                    Err(err) => tracing::warn!("failed to get trending tags: {}", err),
                }
                std::future::pending().await
            },
        )
    })
}
