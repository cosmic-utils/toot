//! Settings context page: timeline display preferences and account switching.

use cosmic::{widget, Element};

use crate::client::Session;
use crate::config::TootConfig;

#[derive(Debug, Clone)]
pub enum Message {
    ToggleHideBoosts(bool),
    ToggleHideReplies(bool),
    SwitchAccount(usize),
    RemoveAccount(usize),
    AddAccount,
}

pub fn view<'a>(config: &'a TootConfig, sessions: &'a [Session], active: usize) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let timeline_settings = widget::settings::section()
        .title("Timeline")
        .add(widget::settings::item(
            "Hide boosts",
            widget::toggler(config.hide_boosts).on_toggle(Message::ToggleHideBoosts),
        ))
        .add(widget::settings::item(
            "Hide replies",
            widget::toggler(config.hide_replies).on_toggle(Message::ToggleHideReplies),
        ));

    let accounts_section = sessions
        .iter()
        .enumerate()
        .fold(
            widget::settings::section().title("Accounts"),
            |section, (index, session)| {
                let mut row: Vec<Element<'a, Message>> = vec![
                    Element::from(widget::text(session.base_url.clone())),
                    Element::from(widget::space::horizontal()),
                ];
                if index != active {
                    row.push(Element::from(
                        widget::button::standard("Switch")
                            .on_press(Message::SwitchAccount(index)),
                    ));
                }
                row.push(Element::from(
                    widget::button::destructive("Remove").on_press(Message::RemoveAccount(index)),
                ));
                section.add(widget::settings::item_row(row))
            },
        )
        .add(widget::button::suggested("Add account").on_press(Message::AddAccount));

    widget::column![timeline_settings, accounts_section]
        .spacing(spacing.space_m)
        .into()
}
