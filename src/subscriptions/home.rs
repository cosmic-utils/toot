use cosmic::iced::{stream, Subscription};
use futures_util::SinkExt;
use megalodon::megalodon::GetHomeTimelineInputOptions;

use crate::{mastodon::Client, pages};

pub fn user_timeline(mastodon: Client, max_id: Option<String>) -> Subscription<pages::home::Message> {
    Subscription::run_with((mastodon, max_id), |(mastodon, max_id)| {
        let mastodon = mastodon.clone();
        let max_id = max_id.clone();
        stream::channel(1, move |mut output: futures_channel::mpsc::Sender<pages::home::Message>| async move {
            let options = GetHomeTimelineInputOptions {
                max_id,
                ..Default::default()
            };

            match mastodon.get_home_timeline(Some(&options)).await {
                Ok(response) => {
                    for status in response.json {
                        if let Err(err) = output
                            .send(pages::home::Message::AppendStatus(status.clone()))
                            .await
                        {
                            tracing::warn!("failed to send post: {}", err);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("failed to get home timeline: {}", err);
                }
            }

            std::future::pending().await
        })
    })
}
