use cosmic::{widget, Element};
use megalodon::entities::{notification::NotificationType, Notification};

use crate::cache::{self, Cache};
use crate::features::status::{self, StatusOptions};

#[derive(Debug, Clone)]
pub enum Message {
    Status(status::Message),
    /// Accept a follow request: (notification id, requesting account id).
    AcceptFollowRequest(String, String),
    /// Reject a follow request: (notification id, requesting account id).
    RejectFollowRequest(String, String),
}

pub fn notification<'a>(notification: &'a Notification, cache: &'a Cache) -> Element<'a, Message> {
    let spacing = cosmic::theme::active().cosmic().spacing;

    let action = notification
        .account
        .as_ref()
        .map(|account| match notification.r#type {
            NotificationType::Mention => format!("{} mentioned you", account.display_name),
            NotificationType::Reblog => format!("{} boosted", account.display_name),
            NotificationType::Favourite => format!("{} liked", account.display_name),
            NotificationType::Follow => {
                format!("{} followed you", account.display_name)
            }
            NotificationType::FollowRequest => {
                format!("{} requested to follow you", account.display_name)
            }
            NotificationType::PollVote => {
                format!("{} voted on a poll", account.display_name)
            }
            NotificationType::Status => format!("{} has posted a status", account.display_name),
            NotificationType::Update => "A post has been edited".to_string(),
            NotificationType::AdminSignup => {
                "Someone signed up (optionally sent to admins)".to_string()
            }
            NotificationType::AdminReport => "A new report has been filed".to_string(),
            NotificationType::PollExpired => "A poll has expired".to_string(),
            NotificationType::Reaction => format!("{} reacted to a status", account.display_name),
            NotificationType::Move => format!("{} moved a status", account.display_name),
            NotificationType::GroupInvited => {
                format!("{} was invited to a group", account.display_name)
            }
            NotificationType::App => format!("{} used an app", account.display_name),
            NotificationType::Quote => format!("{} quoted a status", account.display_name),
            NotificationType::QuotedUpdate => {
                format!("{} updated a quoted status", account.display_name)
            }
            NotificationType::Unknown => "Unknown notification type".to_string(),
        });

    let avatar_url = notification
        .account
        .as_ref()
        .and_then(|account| cache.handles.get(&account.avatar))
        .map(|handle| widget::image(handle).width(20))
        .unwrap_or_else(|| cache::fallback_avatar().width(20));

    let action = action.unwrap_or_else(|| "Unknown notification type".to_string());

    let action = widget::button::custom(
        widget::row![avatar_url, widget::text(action)].spacing(spacing.space_xs),
    )
    .on_press_maybe(notification.account.as_ref().map(|account| {
        Message::Status(status::Message::OpenAccount(account.clone()))
    }));

    let content = notification.status.as_ref().map(|status_data| {
        widget::container(
            status::status(status_data, StatusOptions::new(false, true, false, true), cache)
                .map(Message::Status),
        )
        .padding(spacing.space_xxs)
        .class(cosmic::theme::Container::Dialog(false))
    });

    let follow_request_actions = (notification.r#type == NotificationType::FollowRequest)
        .then_some(notification.account.as_ref())
        .flatten()
        .map(|account| {
            widget::row![
                widget::button::suggested("Accept").on_press(Message::AcceptFollowRequest(
                    notification.id.clone(),
                    account.id.clone(),
                )),
                widget::button::standard("Reject").on_press(Message::RejectFollowRequest(
                    notification.id.clone(),
                    account.id.clone(),
                )),
            ]
            .spacing(spacing.space_xs)
        });

    let content = widget::column![action, follow_request_actions, content].spacing(spacing.space_xs);

    widget::settings::flex_item_row(vec![content.into()])
        .padding(spacing.space_xs)
        .into()
}
