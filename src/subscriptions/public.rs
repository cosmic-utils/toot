use cosmic::iced::{stream, Subscription};
use futures_util::SinkExt;

use crate::{mastodon::Client, pages};

pub fn timeline(mastodon: Client) -> Subscription<pages::public::Message> {
    Subscription::run_with(mastodon, |mastodon| {
        let mastodon = mastodon.clone();
        stream::channel(1, move |mut output: futures_channel::mpsc::Sender<pages::public::Message>| async move {
            match mastodon.get_public_timeline(None).await {
                Ok(response) => {
                    for status in response.json {
                        if let Err(err) = output
                            .send(pages::public::Message::AppendStatus(status.clone()))
                            .await
                        {
                            tracing::warn!("failed to send post: {}", err);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("failed to get public timeline: {}", err);
                }
            }

            std::future::pending().await
        })
    })
}

pub fn local_timeline(mastodon: Client) -> Subscription<pages::public::Message> {
    Subscription::run_with(mastodon, |mastodon| {
        let mastodon = mastodon.clone();
        stream::channel(1, move |mut output: futures_channel::mpsc::Sender<pages::public::Message>| async move {
            match mastodon.get_local_timeline(None).await {
                Ok(response) => {
                    for status in response.json {
                        if let Err(err) = output
                            .send(pages::public::Message::AppendStatus(status.clone()))
                            .await
                        {
                            tracing::warn!("failed to send post: {}", err);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("failed to get local timeline: {}", err);
                }
            }

            std::future::pending().await
        })
    })
}

pub fn remote_timeline(mastodon: Client) -> Subscription<pages::public::Message> {
    Subscription::run_with(mastodon, |mastodon| {
        let mastodon = mastodon.clone();
        stream::channel(1, move |mut output: futures_channel::mpsc::Sender<pages::public::Message>| async move {
            match mastodon.get_public_timeline(None).await {
                Ok(response) => {
                    for status in response.json {
                        if let Err(err) = output
                            .send(pages::public::Message::AppendStatus(status.clone()))
                            .await
                        {
                            tracing::warn!("failed to send post: {}", err);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("failed to get remote timeline: {}", err);
                }
            }

            std::future::pending().await
        })
    })
}
