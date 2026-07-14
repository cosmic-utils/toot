use capitalize::Capitalize;
use cosmic::{
    app::Task,
    iced::{self, alignment::Horizontal, ContentFit, Length},
    widget::{self, image::Handle},
    Apply, Element,
};
use megalodon::entities::Account;
use std::collections::HashMap;

use crate::app;

#[derive(Debug, Clone)]
pub enum Message {
    Open(String),
}

pub fn account<'a>(
    account: &'a Account,
    handles: &'a HashMap<String, Handle>,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;

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

    let settings = (!fields.is_empty()).then_some(widget::settings::section().extend(fields));
    let content = widget::column![stack, display_name, username, bio, joined, info, settings]
        .align_x(Horizontal::Center)
        .width(Length::Fill)
        .spacing(spacing.space_xs);

    widget::settings::flex_item_row(vec![content.into()])
        .padding(spacing.space_xs)
        .into()
}

pub fn update(message: Message) -> Task<app::Message> {
    let tasks = vec![];
    match message {
        Message::Open(url) => {
            if let Err(err) = open::that_detached(url.to_string()) {
                tracing::error!("{err}");
            }
        }
    }
    Task::batch(tasks)
}
