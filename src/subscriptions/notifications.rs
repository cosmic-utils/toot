use cosmic::iced::{stream, Subscription};
use futures_util::SinkExt;
use megalodon::megalodon::GetNotificationsInputOptions;

use crate::{mastodon::Client, pages};

pub fn timeline(mastodon: Client) -> Subscription<pages::notifications::Message> {
    Subscription::run_with(mastodon, |mastodon| {
        let mastodon = mastodon.clone();
        stream::channel(1, move |mut output: futures_channel::mpsc::Sender<pages::notifications::Message>| async move {
            let options = GetNotificationsInputOptions {
                limit: Some(100),
                ..Default::default()
            };

            match mastodon.get_notifications(Some(&options)).await {
                Ok(response) => {
                    for notification in response.json {
                        if let Err(err) = output
                            .send(pages::notifications::Message::AppendNotification(
                                notification.clone(),
                            ))
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

            std::future::pending().await
        })
    })
}
