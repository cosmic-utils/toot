//! Search: accounts, hashtags, and statuses matching a query.

use cosmic::{app::Task, iced::Subscription, widget, Apply, Element};
use megalodon::{
    entities::{Account, Results},
    megalodon::SearchInputOptions,
};

use crate::{
    app,
    cache::Cache,
    client::Client,
    features::{
        status::{self, StatusOptions},
        timeline::{self, Timeline, TimelineKind},
    },
};

pub struct Search {
    mastodon: Client,
    query: String,
    results: Option<Results>,
    tag_timeline: Option<Timeline>,
}

#[derive(Debug, Clone)]
pub enum Message {
    SetClient(Client),
    QueryChanged(String),
    Submit,
    SetResults(Results),
    OpenAccount(Account),
    SelectTag(String),
    Status(status::Message),
    Timeline(timeline::Message),
}

impl Search {
    pub fn new(mastodon: Client) -> Self {
        Self {
            mastodon,
            query: String::new(),
            results: None,
            tag_timeline: None,
        }
    }

    pub fn view<'a>(&'a self, cache: &'a Cache) -> Element<'a, Message> {
        let spacing = cosmic::theme::active().cosmic().spacing;

        let input = widget::text_input("Search accounts, hashtags, statuses", &self.query)
            .on_input(Message::QueryChanged)
            .on_submit(|_| Message::Submit);

        let accounts: Option<Element<_>> = self.results.as_ref().and_then(|results| {
            (!results.accounts.is_empty()).then(|| {
                widget::column(
                    results
                        .accounts
                        .iter()
                        .map(|account| account_row(account, cache))
                        .collect::<Vec<_>>(),
                )
                .spacing(spacing.space_xs)
                .into()
            })
        });

        let hashtags: Option<Element<_>> = self.results.as_ref().and_then(|results| {
            (!results.hashtags.is_empty()).then(|| {
                widget::row(
                    results
                        .hashtags
                        .iter()
                        .map(|tag| {
                            widget::button::suggested(format!("#{}", tag.name))
                                .on_press(Message::SelectTag(tag.name.clone()))
                                .into()
                        })
                        .collect::<Vec<_>>(),
                )
                .spacing(spacing.space_xxs)
                .into()
            })
        });

        let statuses: Option<Element<_>> = self.results.as_ref().and_then(|results| {
            (!results.statuses.is_empty()).then(|| {
                widget::column(
                    results
                        .statuses
                        .iter()
                        .map(|status| {
                            status::status(status, StatusOptions::all(), cache)
                                .map(Message::Status)
                        })
                        .collect::<Vec<_>>(),
                )
                .into()
            })
        });

        let tag_timeline = self
            .tag_timeline
            .as_ref()
            .map(|timeline| timeline.view(cache).map(Message::Timeline));

        widget::scrollable(
            widget::column![input, accounts, hashtags, statuses, tag_timeline]
                .spacing(spacing.space_s),
        )
        .apply(widget::container)
        .max_width(700)
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
                    let mastodon = self.mastodon.clone();
                    return cosmic::task::future(async move {
                        match mastodon.search(query, Some(&SearchInputOptions::default())).await {
                            Ok(response) => {
                                app::Message::Search(Message::SetResults(response.json))
                            }
                            Err(err) => app::Message::Error(format!("Search failed: {err}")),
                        }
                    });
                }
            }
            Message::SetResults(results) => self.results = Some(results),
            Message::OpenAccount(account) => {
                return cosmic::task::message(app::Message::ToggleContextPage(
                    app::ContextPage::Account(account),
                ));
            }
            Message::SelectTag(name) => {
                self.tag_timeline = Some(Timeline::new(self.mastodon.clone(), TimelineKind::Tag(name)));
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
        self.tag_timeline
            .as_ref()
            .map(|timeline| timeline.subscription().map(Message::Timeline))
            .unwrap_or(Subscription::none())
    }
}

fn account_row<'a>(account: &'a Account, cache: &'a Cache) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    widget::button::custom(
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
        .align_y(cosmic::iced::Alignment::Center),
    )
    .on_press(Message::OpenAccount(account.clone()))
    .into()
}
