use std::str::FromStr;

use cosmic::{
    app::Task,
    iced::{mouse::Interaction, Alignment, Length},
    iced_widget::scrollable::{Direction, Scrollbar},
    widget, Apply, Element,
};
use mastodon_async::{
    prelude::{Account, Status, StatusId},
    NewStatus,
};
use reqwest::Url;

use crate::{
    app,
    utils::{self, Cache},
};

#[derive(Debug, Clone)]
pub enum Message {
    OpenAccount(Account),
    ExpandStatus(StatusId),
    Reply(StatusId, String),
    Favorite(StatusId, bool),
    Boost(StatusId, bool),
    OpenLink(Url),
}

#[derive(Debug, Copy, Clone)]
pub struct StatusOptions {
    media: bool,
    tags: bool,
    actions: bool,
    expand: bool,
}

impl StatusOptions {
    pub fn new(media: bool, tags: bool, actions: bool, expand: bool) -> Self {
        Self {
            media,
            tags,
            actions,
            expand,
        }
    }

    pub fn all() -> StatusOptions {
        StatusOptions::new(true, true, true, true)
    }

    pub fn none() -> StatusOptions {
        StatusOptions::new(false, false, false, false)
    }
}

pub fn status<'a>(
    status: &'a Status,
    options: StatusOptions,
    cache: &'a Cache,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    let reblog_button = reblog_button(cache, status);
    let status = status
        .reblog
        .as_ref()
        .map(|reblog| cache.statuses.get(&reblog.id.to_string()).unwrap_or(reblog))
        .unwrap_or(status);

    widget::column()
        .push_maybe(reblog_button)
        .push(header(status, cache))
        .push(content(status, options))
        .push_maybe(card(status, cache))
        .push_maybe(media(status, cache, options))
        .push_maybe(tags(status, options))
        .push_maybe(actions(status, options))
        .padding(spacing.space_xs)
        .spacing(spacing.space_xs)
        .width(Length::Fill)
        .into()
}

fn card<'a>(status: &'a Status, cache: &'a Cache) -> Option<Element<'a, Message>> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    status.card.as_ref().map(|card| {
        widget::column()
            .push_maybe(card.image.as_ref().map(|image| {
                Url::from_str(image)
                    .ok()
                    .map(|url| {
                        cache
                            .handles
                            .get(&url)
                            .map(widget::image)
                            .unwrap_or(utils::fallback_avatar())
                    })
                    .unwrap_or(utils::fallback_avatar())
            }))
            .push(
                widget::column()
                    .push(widget::text::title4(&card.title))
                    .push(widget::text(&card.description))
                    .spacing(spacing.space_xs)
                    .padding(spacing.space_xs),
            )
            .apply(widget::container)
            .class(cosmic::style::Container::Dialog)
            .apply(widget::button::custom)
            .class(cosmic::style::Button::Image)
            .on_press(Message::OpenLink(card.url.clone()))
            .into()
    })
}

pub fn update(message: Message) -> Task<app::Message> {
    match message {
        Message::OpenAccount(account) => cosmic::task::message(app::Message::ToggleContextPage(
            app::ContextPage::Account(account),
        )),
        Message::ExpandStatus(id) => cosmic::task::message(app::Message::ToggleContextPage(
            app::ContextPage::Status(id),
        )),
        Message::Reply(status_id, username) => {
            let new_status = NewStatus {
                in_reply_to_id: Some(status_id.to_string()),
                status: Some(format!("@{} ", username)),
                ..Default::default()
            };
            cosmic::task::message(app::Message::Dialog(app::DialogAction::Open(
                app::Dialog::Reply(new_status),
            )))
        }
        Message::Favorite(status_id, favorited) => cosmic::task::message(app::Message::Status(
            Message::Favorite(status_id, favorited),
        )),
        Message::Boost(status_id, boosted) => {
            cosmic::task::message(app::Message::Status(Message::Boost(status_id, boosted)))
        }
        Message::OpenLink(url) => cosmic::task::message(app::Message::Open(url.to_string())),
    }
}

fn actions(status: &Status, options: StatusOptions) -> Option<Element<'_, Message>> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let actions = (options.actions).then_some({
        widget::row()
            .push(
                widget::button::icon(widget::icon::from_name("mail-replied-symbolic"))
                    .label(status.replies_count.to_string())
                    .on_press(Message::Reply(
                        status.id.clone(),
                        status.account.username.clone(),
                    )),
            )
            .push(
                widget::button::icon(widget::icon::from_name("emblem-shared-symbolic"))
                    .label(status.reblogs_count.to_string())
                    .class(
                        status
                            .reblogged
                            .map(|reblogged| {
                                if reblogged {
                                    cosmic::theme::Button::Suggested
                                } else {
                                    cosmic::theme::Button::Icon
                                }
                            })
                            .unwrap_or(cosmic::theme::Button::Icon),
                    )
                    .on_press_maybe(
                        status
                            .reblogged
                            .map(|reblogged| Message::Boost(status.id.clone(), reblogged)),
                    ),
            )
            .push(
                widget::button::icon(widget::icon::from_name("starred-symbolic"))
                    .label(status.favourites_count.to_string())
                    .class(
                        status
                            .favourited
                            .map(|favourited| {
                                if favourited {
                                    cosmic::theme::Button::Suggested
                                } else {
                                    cosmic::theme::Button::Icon
                                }
                            })
                            .unwrap_or(cosmic::theme::Button::Icon),
                    )
                    .on_press_maybe(
                        status
                            .favourited
                            .map(|favourited| Message::Favorite(status.id.clone(), favourited)),
                    ),
            )
            .spacing(spacing.space_xs)
            .into()
    });
    actions
}

fn media<'a>(
    status: &'a Status,
    cache: &'a Cache,
    options: StatusOptions,
) -> Option<cosmic::iced_widget::Scrollable<'a, Message, cosmic::Theme>> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let attachments = status
        .media_attachments
        .iter()
        .map(|media| {
            widget::button::image(
                cache
                    .handles
                    .get(&media.preview_url)
                    .cloned()
                    .unwrap_or(crate::utils::fallback_handle()),
            )
            .on_press_maybe(media.url.as_ref().cloned().map(Message::OpenLink))
            .into()
        })
        .collect::<Vec<Element<Message>>>();

    let media = (!status.media_attachments.is_empty() && options.media).then_some({
        widget::scrollable(widget::row().extend(attachments).spacing(spacing.space_xxs))
            .direction(Direction::Horizontal(Scrollbar::new()))
    });
    media
}

fn tags(status: &Status, options: StatusOptions) -> Option<Element<'_, Message>> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let tags: Option<Element<_>> = (!status.tags.is_empty() && options.tags).then(|| {
        widget::row()
            .spacing(spacing.space_xxs)
            .extend(
                status
                    .tags
                    .iter()
                    .map(|tag| {
                        widget::button::suggested(format!("#{}", tag.name.clone()))
                            .on_press_maybe(Url::from_str(&tag.url).map(Message::OpenLink).ok())
                            .into()
                    })
                    .collect::<Vec<Element<Message>>>(),
            )
            .into()
    });
    tags
}

fn header<'a>(
    status: &'a Status,
    cache: &'a Cache,
) -> cosmic::iced_widget::Row<'a, Message, cosmic::Theme> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let header = widget::row()
        .push(
            widget::button::image(
                cache
                    .handles
                    .get(&status.account.avatar)
                    .cloned()
                    .unwrap_or(crate::utils::fallback_handle()),
            )
            .width(50)
            .height(50)
            .on_press(Message::OpenAccount(status.account.clone())),
        )
        .push(
            widget::column()
                .push(widget::text(status.account.display_name.clone()).size(18))
                .push(
                    widget::button::link(format!("@{}", status.account.username.clone()))
                        .on_press(Message::OpenAccount(status.account.clone())),
                ),
        )
        .align_y(Alignment::Center)
        .spacing(spacing.space_xs);
    header
}

fn content(status: &Status, options: StatusOptions) -> Element<'_, Message> {
    let mut status_text: Element<_> = widget::text(
        html2text::config::rich()
            .string_from_read(status.content.as_bytes(), 700)
            .unwrap(),
    )
    .into();

    if options.expand {
        status_text = widget::MouseArea::new(status_text)
            .on_press(Message::ExpandStatus(status.id.clone()))
            .interaction(Interaction::Pointer)
            .into();
    }
    status_text
}

fn reblog_button<'a>(cache: &'a Cache, status: &'a Status) -> Option<widget::Button<'a, Message>> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    (status.reblog.is_some()).then_some(
        widget::button::custom(
            widget::row()
                .push(
                    cache
                        .handles
                        .get(&status.account.avatar)
                        .map(|avatar| widget::image(avatar).width(20).height(20))
                        .unwrap_or(crate::utils::fallback_avatar().width(20).height(20)),
                )
                .push(widget::text(format!(
                    "{} boosted",
                    status.account.display_name
                )))
                .spacing(spacing.space_xs),
        )
        .on_press(Message::OpenAccount(status.account.clone())),
    )
}
