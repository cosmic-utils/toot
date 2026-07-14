//! Compose dialog: new top-level posts and replies, with a content-warning
//! toggle, a visibility picker, and a live character counter.

use cosmic::{iced::Length, widget, Element};
use megalodon::entities::status::StatusVisibility;

use crate::app::{Dialog, DialogAction, Message};
use crate::fl;

/// State for the compose dialog. A new top-level post when `in_reply_to_id`
/// is `None`, a reply otherwise. `text` seeds the dialog's text editor when
/// the dialog opens (e.g. `@user `) and is overwritten with the editor's
/// final contents right before submitting.
#[derive(Debug, Clone)]
pub struct State {
    pub in_reply_to_id: Option<String>,
    pub text: Option<String>,
    pub content_warning: bool,
    pub spoiler_text: String,
    pub visibility: StatusVisibility,
}

impl Default for State {
    fn default() -> Self {
        Self {
            in_reply_to_id: None,
            text: None,
            content_warning: false,
            spoiler_text: String::new(),
            visibility: StatusVisibility::Public,
        }
    }
}

impl State {
    pub fn reply(status_id: String, username: String) -> Self {
        Self {
            in_reply_to_id: Some(status_id),
            text: Some(format!("@{} ", username)),
            ..Default::default()
        }
    }
}

const VISIBILITIES: [StatusVisibility; 4] = [
    StatusVisibility::Public,
    StatusVisibility::Unlisted,
    StatusVisibility::Private,
    StatusVisibility::Direct,
];

fn visibility_label(visibility: &StatusVisibility) -> &'static str {
    match visibility {
        StatusVisibility::Public => "Public",
        StatusVisibility::Unlisted => "Unlisted",
        StatusVisibility::Private => "Followers only",
        StatusVisibility::Direct => "Direct",
        StatusVisibility::Local => "Local",
    }
}

/// Build the compose dialog. `reply_preview` renders the status being
/// replied to (if any); `editor` is the app-level text editor content shared
/// across compose sessions; `max_characters` comes from the instance's
/// reported status length limit.
pub fn view<'a>(
    state: &'a State,
    reply_preview: Option<Element<'a, Message>>,
    editor: &'a widget::text_editor::Content,
    max_characters: u32,
) -> widget::Dialog<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let labels: Vec<&str> = VISIBILITIES.iter().map(visibility_label).collect();
    let selected = VISIBILITIES.iter().position(|v| *v == state.visibility);

    let remaining = max_characters as i64 - editor.text().chars().count() as i64;
    let title = if state.in_reply_to_id.is_some() {
        fl!("reply")
    } else {
        fl!("new-post")
    };

    widget::dialog()
        .title(title)
        .control(
            widget::container(
                widget::scrollable(
                    widget::column![
                        reply_preview,
                        widget::row![
                            widget::dropdown(labels, selected, {
                                let state = state.clone();
                                move |index| {
                                    let mut state = state.clone();
                                    state.visibility = VISIBILITIES[index].clone();
                                    Message::Dialog(DialogAction::Update(Dialog::Compose(state)))
                                }
                            }),
                            widget::space::horizontal(),
                            widget::toggler(state.content_warning)
                                .label(fl!("content-warning"))
                                .on_toggle({
                                    let state = state.clone();
                                    move |value| {
                                        let mut state = state.clone();
                                        state.content_warning = value;
                                        Message::Dialog(DialogAction::Update(Dialog::Compose(
                                            state,
                                        )))
                                    }
                                }),
                        ]
                        .spacing(spacing.space_xs)
                        .align_y(cosmic::iced::Alignment::Center),
                        state.content_warning.then(|| {
                            widget::text_input(fl!("content-warning-placeholder"), &state.spoiler_text)
                                .on_input({
                                    let state = state.clone();
                                    move |value| {
                                        let mut state = state.clone();
                                        state.spoiler_text = value;
                                        Message::Dialog(DialogAction::Update(Dialog::Compose(
                                            state,
                                        )))
                                    }
                                })
                        }),
                        widget::text_editor(editor)
                            .placeholder(fl!("whats-happening"))
                            .height(160.)
                            .padding(spacing.space_s)
                            .on_action(Message::EditorAction),
                        widget::text::caption(remaining.to_string()),
                    ]
                    .spacing(spacing.space_xs),
                )
                .width(Length::Fill),
            )
            .height(Length::Fixed(420.0))
            .width(Length::Fill),
        )
        .primary_action(
            widget::button::suggested(fl!("post")).on_press_maybe(
                (!editor.text().trim().is_empty() && remaining >= 0)
                    .then_some(Message::Dialog(DialogAction::Complete)),
            ),
        )
        .secondary_action(
            widget::button::standard(fl!("cancel")).on_press(Message::Dialog(DialogAction::Close)),
        )
}
