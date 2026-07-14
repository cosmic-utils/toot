//! Real-time streaming of the authenticated user's events (new posts,
//! notifications, deletions), pushed into the relevant feature's message type.

use cosmic::iced::{stream, Subscription};
use futures_util::SinkExt;
use megalodon::streaming::Message as StreamMessage;

use crate::{app, client::Client, features::notifications, features::timeline};

pub fn stream_user_events(mastodon: Client) -> Subscription<app::Message> {
    Subscription::run_with(mastodon, |mastodon| {
        let mastodon = mastodon.clone();
        stream::channel(1, |output: futures_channel::mpsc::Sender<app::Message>| async move {
            let streaming = mastodon.user_streaming().await;

            streaming
                .listen(Box::new(move |message| {
                    let mut output = output.clone();
                    Box::pin(async move {
                        match message {
                            StreamMessage::Update(status) => {
                                if let Err(err) = output
                                    .send(app::Message::Home(timeline::Message::PrependStatus(
                                        status,
                                    )))
                                    .await
                                {
                                    tracing::warn!("failed to send post: {}", err);
                                }
                            }
                            StreamMessage::Notification(notification) => {
                                if let Err(err) = output
                                    .send(app::Message::Notifications(
                                        notifications::Message::PrependNotification(notification),
                                    ))
                                    .await
                                {
                                    tracing::warn!("failed to send post: {}", err);
                                }
                            }
                            StreamMessage::Delete(id) => {
                                if let Err(err) = output
                                    .send(app::Message::Home(timeline::Message::DeleteStatus(id)))
                                    .await
                                {
                                    tracing::warn!("failed to send post: {}", err);
                                }
                            }
                            StreamMessage::Conversation(_)
                            | StreamMessage::StatusUpdate(_)
                            | StreamMessage::Heartbeat() => (),
                        }
                    })
                }))
                .await;

            std::future::pending().await
        })
    })
}
