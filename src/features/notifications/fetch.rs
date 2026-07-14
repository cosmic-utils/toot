use cosmic::iced::{stream, Subscription};
use futures_util::SinkExt;
use megalodon::megalodon::GetNotificationsInputOptions;

use crate::client::Client;

use super::Message;

pub fn timeline(mastodon: Client, max_id: Option<String>) -> Subscription<Message> {
    Subscription::run_with((mastodon, max_id), |(mastodon, max_id)| {
        let mastodon = mastodon.clone();
        let max_id = max_id.clone();
        stream::channel(1, move |mut output: futures_channel::mpsc::Sender<Message>| async move {
            let options = GetNotificationsInputOptions {
                limit: Some(30),
                max_id,
                ..Default::default()
            };

            match mastodon.get_notifications(Some(&options)).await {
                Ok(response) => {
                    for notification in response.json {
                        if let Err(err) = output
                            .send(Message::AppendNotification(notification.clone()))
                            .await
                        {
                            tracing::warn!("failed to send post: {}", err);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("failed to get notifications: {}", err);
                }
            }
            if let Err(err) = output.send(Message::LoadComplete).await {
                tracing::warn!("failed to send load-complete: {}", err);
            }

            std::future::pending().await
        })
    })
}
