//! Settings context page: timeline display preferences and account switching.

use cosmic::{widget, Element};

use crate::client::Session;
use crate::config::{FeedDensity, ThemeMode, TootConfig};

#[derive(Debug, Clone)]
pub enum Message {
    ToggleHideBoosts(bool),
    ToggleHideReplies(bool),
    SetFeedDensity(FeedDensity),
    SetThemeMode(ThemeMode),
    SwitchAccount(usize),
    RemoveAccount(usize),
    AddAccount,
}

pub fn view<'a>(
    config: &'a TootConfig,
    sessions: &'a [Session],
    active: usize,
) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let density_labels: Vec<&str> = FeedDensity::ALL
        .iter()
        .map(|density| density.label())
        .collect();
    let density_selected = FeedDensity::ALL
        .iter()
        .position(|density| *density == config.feed_density);

    let theme_labels: Vec<&str> = ThemeMode::ALL.iter().map(|mode| mode.label()).collect();
    let theme_selected = ThemeMode::ALL
        .iter()
        .position(|mode| *mode == config.theme_mode);

    let appearance_settings =
        widget::settings::section()
            .title("Appearance")
            .add(widget::settings::item(
                "Theme",
                widget::column![widget::dropdown(theme_labels, theme_selected, |index| {
                    Message::SetThemeMode(ThemeMode::ALL[index])
                }),]
                .spacing(spacing.space_xxs)
                .align_x(cosmic::iced::Alignment::End),
            ));

    let timeline_settings = widget::settings::section()
        .title("Timeline")
        .add(widget::settings::item(
            "Post display",
            widget::dropdown(density_labels, density_selected, |index| {
                Message::SetFeedDensity(FeedDensity::ALL[index])
            }),
        ))
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
                        widget::button::standard("Switch").on_press(Message::SwitchAccount(index)),
                    ));
                }
                row.push(Element::from(
                    widget::button::destructive("Remove").on_press(Message::RemoveAccount(index)),
                ));
                section.add(widget::settings::item_row(row))
            },
        )
        .add(widget::button::suggested("Add account").on_press(Message::AddAccount));

    widget::column![appearance_settings, timeline_settings, accounts_section]
        .spacing(spacing.space_m)
        .into()
}
