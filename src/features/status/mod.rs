use cosmic::{
    app::Task,
    iced::widget::scrollable::{Direction, Scrollbar},
    iced::{mouse::Interaction, Alignment, Length},
    widget, Apply, Element,
};
use megalodon::entities::{Account, Status};

use crate::{
    app,
    cache::{self, Cache},
    config::FeedDensity,
    features::compose,
};

#[derive(Debug, Clone)]
pub enum Message {
    OpenAccount(Account),
    ExpandStatus(String),
    Reply(String, String),
    Favorite(String, bool),
    Boost(String, bool),
    Bookmark(String, bool),
    OpenLink(String),
    /// Show an image attachment in an overlay: (cached preview URL, original URL).
    ViewImage(String, String),
    /// Request to delete one of the authenticated user's own statuses;
    /// opens a confirmation dialog rather than deleting immediately.
    Delete(String),
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

    let density = cache.feed_density;

    widget::column![
        reblog_button,
        header(status, cache, density),
        content(status, options),
        card(status, cache, density),
        media(status, cache, options, density),
        tags(status, options),
        actions(status, options, cache),
    ]
    .padding(spacing.space_xs)
    .spacing(spacing.space_xs)
    .width(Length::Fill)
    .into()
}

fn card<'a>(
    status: &'a Status,
    cache: &'a Cache,
    density: FeedDensity,
) -> Option<Element<'a, Message>> {
    if density == FeedDensity::TextOnly {
        return None;
    }
    let compact = density == FeedDensity::Compact;

    let spacing = cosmic::theme::active().cosmic().spacing;
    status.card.as_ref().map(|card| {
        let avatar = card.image.as_ref().map(|image| {
            let fallback = cache::fallback_avatar();
            let handle = cache
                .handles
                .get(image)
                .map(widget::image)
                .unwrap_or(fallback);
            if compact {
                handle
                    .width(150.0)
                    .height(Length::Fill)
                    .content_fit(cosmic::iced::ContentFit::Cover)
            } else {
                handle
            }
        });

        let text = widget::column![
            widget::text::title4(&card.title).wrapping(cosmic::iced::core::text::Wrapping::Word),
            widget::text(&card.description).wrapping(cosmic::iced::core::text::Wrapping::Word),
        ]
        .spacing(spacing.space_xs)
        .padding(spacing.space_xs)
        .width(Length::Fill);

        let content: Element<'_, Message> = if compact {
            widget::row![avatar, text]
                .spacing(spacing.space_xs)
                .align_y(Alignment::Center)
                .width(Length::Fill)
                .into()
        } else {
            widget::column![avatar, text].into()
        };

        content
            .apply(widget::container)
            .width(Length::Fill)
            .class(cosmic::style::Container::Dialog(false))
            .apply(widget::button::custom)
            .width(Length::Fill)
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
            let state = compose::State::reply(status_id, username);
            cosmic::task::message(app::Message::Dialog(app::DialogAction::Open(
                app::Dialog::Compose(state),
            )))
        }
        Message::Favorite(status_id, favorited) => cosmic::task::message(app::Message::Status(
            Message::Favorite(status_id, favorited),
        )),
        Message::Boost(status_id, boosted) => {
            cosmic::task::message(app::Message::Status(Message::Boost(status_id, boosted)))
        }
        Message::Bookmark(status_id, bookmarked) => cosmic::task::message(app::Message::Status(
            Message::Bookmark(status_id, bookmarked),
        )),
        Message::OpenLink(url) => cosmic::task::message(app::Message::Open(url.to_string())),
        Message::ViewImage(preview_url, original_url) => {
            cosmic::task::message(app::Message::Dialog(app::DialogAction::Open(
                app::Dialog::Image(preview_url, original_url),
            )))
        }
        Message::Delete(status_id) => cosmic::task::message(app::Message::Dialog(
            app::DialogAction::Open(app::Dialog::DeleteStatus(status_id)),
        )),
    }
}

fn actions<'a>(
    status: &'a Status,
    options: StatusOptions,
    cache: &'a Cache,
) -> Option<Element<'a, Message>> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let delete_button = cache.is_me(&status.account.id).then(|| {
        widget::button::icon(widget::icon::from_name("user-trash-symbolic"))
            .on_press(Message::Delete(status.id.clone()))
    });

    let actions = (options.actions).then_some({
        widget::row![
            widget::button::icon(widget::icon::from_name("mail-replied-symbolic"))
                .label(status.replies_count.to_string())
                .on_press(Message::Reply(
                    status.id.clone(),
                    status.account.username.clone(),
                )),
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
            widget::button::icon(widget::icon::from_name("bookmark-new-symbolic"))
                .class(
                    status
                        .bookmarked
                        .map(|bookmarked| {
                            if bookmarked {
                                cosmic::theme::Button::Suggested
                            } else {
                                cosmic::theme::Button::Icon
                            }
                        })
                        .unwrap_or(cosmic::theme::Button::Icon),
                )
                .on_press_maybe(
                    status
                        .bookmarked
                        .map(|bookmarked| Message::Bookmark(status.id.clone(), bookmarked)),
                ),
            widget::space::horizontal(),
        ]
        .push_maybe(delete_button)
        .spacing(spacing.space_xs)
        .into()
    });
    actions
}

fn media<'a>(
    status: &'a Status,
    cache: &'a Cache,
    options: StatusOptions,
    density: FeedDensity,
) -> Option<cosmic::iced::widget::Scrollable<'a, Message, cosmic::Theme>> {
    if density == FeedDensity::TextOnly {
        return None;
    }
    let compact = density == FeedDensity::Compact;

    let spacing = cosmic::theme::active().cosmic().spacing;

    let attachments = status
        .media_attachments
        .iter()
        .map(|media| {
            let is_picture = matches!(
                media.r#type,
                megalodon::entities::attachment::AttachmentType::Image
                    | megalodon::entities::attachment::AttachmentType::Gifv
            );
            let on_press = match &media.preview_url {
                Some(preview_url) if is_picture => {
                    Message::ViewImage(preview_url.clone(), media.url.clone())
                }
                _ => Message::OpenLink(media.url.clone()),
            };

            let button = widget::button::image(
                media
                    .preview_url
                    .as_ref()
                    .and_then(|url| cache.handles.get(url))
                    .cloned()
                    .unwrap_or(crate::cache::fallback_handle()),
            )
            .on_press(on_press);
            if compact {
                button.width(80.0).height(80.0).into()
            } else {
                button.into()
            }
        })
        .collect::<Vec<Element<Message>>>();

    let media = (!status.media_attachments.is_empty() && options.media).then_some({
        widget::scrollable(widget::row(attachments).spacing(spacing.space_xxs))
            .direction(Direction::Horizontal(Scrollbar::new()))
    });
    media
}

fn tags(status: &Status, options: StatusOptions) -> Option<Element<'_, Message>> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let tags: Option<Element<_>> = (!status.tags.is_empty() && options.tags).then(|| {
        let tags = status
            .tags
            .iter()
            .map(|tag| {
                widget::button::suggested(format!("#{}", tag.name.clone()))
                    .on_press(Message::OpenLink(tag.url.clone()))
                    .into()
            })
            .collect::<Vec<Element<Message>>>();
        widget::flex_row(tags).spacing(spacing.space_xxs).into()
    });
    tags
}

fn header<'a>(
    status: &'a Status,
    cache: &'a Cache,
    density: FeedDensity,
) -> cosmic::widget::Row<'a, Message, cosmic::Theme> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    let avatar_size = if density == FeedDensity::Full {
        50.0
    } else {
        36.0
    };

    let header = widget::row![
        widget::button::image(
            cache
                .handles
                .get(&status.account.avatar)
                .cloned()
                .unwrap_or(crate::cache::fallback_handle()),
        )
        .width(avatar_size)
        .height(avatar_size)
        .on_press(Message::OpenAccount(status.account.clone())),
        widget::column![]
            .push_maybe(
                (!status.account.display_name.is_empty())
                    .then(|| { widget::text(status.account.display_name.clone()).size(18) })
            )
            .push(
                widget::button::link(format!("@{}", status.account.username.clone()))
                    .on_press(Message::OpenAccount(status.account.clone())),
            )
            .align_x(Alignment::Center)
            .spacing(spacing.space_xs),
    ]
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
            widget::row![
                cache
                    .handles
                    .get(&status.account.avatar)
                    .map(|avatar| widget::image(avatar).width(20).height(20))
                    .unwrap_or(crate::cache::fallback_avatar().width(20).height(20)),
                widget::text(format!("{} boosted", status.account.display_name)),
            ]
            .spacing(spacing.space_xs),
        )
        .on_press(Message::OpenAccount(status.account.clone())),
    )
}
