use cosmic::iced::{stream, Subscription};
use futures_util::SinkExt;
use megalodon::megalodon::{
    GetBookmarksInputOptions, GetFavouritesInputOptions, GetHomeTimelineInputOptions,
    GetListTimelineInputOptions, GetLocalTimelineInputOptions, GetPublicTimelineInputOptions,
    GetTagTimelineInputOptions,
};

use crate::client::Client;

use super::{Message, TimelineKind};

pub fn timeline(mastodon: Client, kind: TimelineKind, max_id: Option<String>) -> Subscription<Message> {
    Subscription::run_with((mastodon, kind, max_id), |(mastodon, kind, max_id)| {
        let mastodon = mastodon.clone();
        let kind = kind.clone();
        let max_id = max_id.clone();
        stream::channel(1, move |mut output: futures_channel::mpsc::Sender<Message>| async move {
            let result = match &kind {
                TimelineKind::Home => {
                    let options = GetHomeTimelineInputOptions {
                        max_id,
                        ..Default::default()
                    };
                    mastodon
                        .get_home_timeline(Some(&options))
                        .await
                        .map(|response| response.json)
                }
                TimelineKind::Local => {
                    let options = GetLocalTimelineInputOptions {
                        max_id,
                        ..Default::default()
                    };
                    mastodon
                        .get_local_timeline(Some(&options))
                        .await
                        .map(|response| response.json)
                }
                TimelineKind::Public => {
                    let options = GetPublicTimelineInputOptions {
                        max_id,
                        ..Default::default()
                    };
                    mastodon
                        .get_public_timeline(Some(&options))
                        .await
                        .map(|response| response.json)
                }
                TimelineKind::Federated => {
                    // megalodon has no "remote only" flag on the public timeline endpoint,
                    // so federated posts are derived by filtering out local ones: a status's
                    // account `acct` only carries an `@instance` suffix for remote accounts.
                    let options = GetPublicTimelineInputOptions {
                        max_id,
                        ..Default::default()
                    };
                    mastodon.get_public_timeline(Some(&options)).await.map(|response| {
                        response
                            .json
                            .into_iter()
                            .filter(|status| status.account.acct.contains('@'))
                            .collect()
                    })
                }
                TimelineKind::Favorites => {
                    let options = GetFavouritesInputOptions {
                        max_id,
                        ..Default::default()
                    };
                    mastodon
                        .get_favourites(Some(&options))
                        .await
                        .map(|response| response.json)
                }
                TimelineKind::Bookmarks => {
                    let options = GetBookmarksInputOptions {
                        max_id,
                        ..Default::default()
                    };
                    mastodon
                        .get_bookmarks(Some(&options))
                        .await
                        .map(|response| response.json)
                }
                TimelineKind::Tag(tag) => {
                    let options = GetTagTimelineInputOptions {
                        max_id,
                        ..Default::default()
                    };
                    mastodon
                        .get_tag_timeline(tag.clone(), Some(&options))
                        .await
                        .map(|response| response.json)
                }
                TimelineKind::List(id) => {
                    let options = GetListTimelineInputOptions {
                        max_id,
                        ..Default::default()
                    };
                    mastodon
                        .get_list_timeline(id.clone(), Some(&options))
                        .await
                        .map(|response| response.json)
                }
            };

            match result {
                Ok(statuses) => {
                    for status in statuses {
                        if let Err(err) = output.send(Message::AppendStatus(status)).await {
                            tracing::warn!("failed to send post: {}", err);
                        }
                    }
                }
                Err(err) => {
                    tracing::warn!("failed to get {:?} timeline: {}", kind, err);
                }
            }

            std::future::pending().await
        })
    })
}
