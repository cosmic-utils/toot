//! Account profile view: bio/stats plus follow/mute/block relationship actions.

use capitalize::Capitalize;
use cosmic::{
    app::Task,
    iced::{self, alignment::Horizontal, ContentFit, Length},
    widget, Apply, Element,
};
use megalodon::entities::{Account, Relationship};

use crate::app;
use crate::cache::Cache;

#[derive(Debug, Clone)]
pub enum Message {
    Open(String),
    Follow(String, bool),
    Mute(String, bool),
    Block(String, bool),
}

pub fn account<'a>(account: &'a Account, cache: &'a Cache) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;
    let handles = &cache.handles;
    let relationship = cache.relationships.get(&account.id);
    let is_me = cache.is_me(&account.id);

    let header = handles.get(&account.header).map(|handle| {
        widget::image(handle)
            .content_fit(ContentFit::Cover)
            .height(120.0)
    });
    let avatar = handles.get(&account.avatar).map(|handle| {
        widget::container(
            widget::button::image(handle)
                .on_press(Message::Open(account.avatar.clone()))
                .width(100),
        )
        .center(Length::Fill)
    });
    let stack = iced::widget::stack!(header, avatar);
    let display_name = widget::text(&account.display_name).size(18);
    let username = widget::button::link(format!("@{}", account.username))
        .on_press(Message::Open(account.url.clone()));
    let bio = (!account.note.is_empty()).then_some(widget::text(
        html2text::config::rich()
            .string_from_read(account.note.as_bytes(), 700)
            .unwrap(),
    ));
    let joined = widget::text::caption(format!(
        "Joined on {}",
        account.created_at.format("%d %b %Y")
    ));
    let fields: Vec<Element<_>> = account
        .fields
        .iter()
        .map(|field| {
            let value = html2text::config::rich()
                .string_from_read(field.value.as_bytes(), 700)
                .unwrap();
            widget::column![
                widget::text(field.name.capitalize()),
                widget::text(value.clone()).class(cosmic::style::Text::Accent),
            ]
            .width(Length::Fill)
            .apply(widget::button::custom)
            .class(cosmic::style::Button::Icon)
            .on_press(Message::Open(value.clone()))
            .into()
        })
        .collect();
    let followers = widget::column![
        widget::text::text("Followers"),
        widget::text::title3(account.followers_count.to_string()),
    ];
    let following = widget::column![
        widget::text::text("Following"),
        widget::text::title3(account.following_count.to_string()),
    ]
    .width(Length::FillPortion(1))
    .align_x(Horizontal::Center);
    let statuses = widget::column![
        widget::text::text("Posts"),
        widget::text::title3(account.statuses_count.to_string()),
    ]
    .width(Length::FillPortion(1))
    .align_x(Horizontal::Center);

    let info = widget::container(
        widget::row![
            followers,
            widget::divider::vertical::light().height(Length::Fixed(50.)),
            following,
            widget::divider::vertical::light().height(Length::Fixed(50.)),
            statuses,
        ]
        .padding(spacing.space_xs)
        .spacing(spacing.space_xs),
    )
    .class(cosmic::style::Container::Card);

    let relationship_actions = (!is_me).then(|| relationship_actions(account, relationship));

    let settings = (!fields.is_empty()).then_some(widget::settings::section().extend(fields));
    let content = widget::column![
        stack,
        display_name,
        username,
        relationship_actions,
        bio,
        joined,
        info,
        settings
    ]
    .align_x(Horizontal::Center)
    .width(Length::Fill)
    .spacing(spacing.space_xs);

    widget::settings::flex_item_row(vec![content.into()])
        .padding(spacing.space_xs)
        .into()
}

fn relationship_actions<'a>(
    account: &'a Account,
    relationship: Option<&'a Relationship>,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let following = relationship.is_some_and(|r| r.following);
    let muting = relationship.is_some_and(|r| r.muting);
    let blocking = relationship.is_some_and(|r| r.blocking);

    widget::row![
        widget::button::text(if following { "Unfollow" } else { "Follow" })
            .class(if following {
                cosmic::theme::Button::Standard
            } else {
                cosmic::theme::Button::Suggested
            })
            .on_press(Message::Follow(account.id.clone(), following)),
        widget::button::text(if muting { "Unmute" } else { "Mute" })
            .on_press(Message::Mute(account.id.clone(), muting)),
        widget::button::text(if blocking { "Unblock" } else { "Block" })
            .class(cosmic::theme::Button::Destructive)
            .on_press(Message::Block(account.id.clone(), blocking)),
    ]
    .spacing(spacing.space_xs)
    .into()
}

/// Handles [`Message::Open`] directly; [`Message::Follow`]/[`Message::Mute`]/
/// [`Message::Block`] carry no view-only behavior of their own — they're
/// intercepted in [`app::AppModel::update`] to perform the actual API call.
pub fn update(message: Message) -> Task<app::Message> {
    if let Message::Open(url) = message {
        if let Err(err) = open::that_detached(&url) {
            tracing::error!("{err}");
        }
    }
    Task::none()
}
