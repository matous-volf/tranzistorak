use crate::model::Track;
use crate::query::Fetched;
use crate::query::spotify::ToSearchQuery;
use crate::utils::AsyncIterator;
use crate::{query, youtube};
use rspotify::clients::BaseClient;
use rspotify::clients::pagination::Paginator;
use rspotify::http::HttpError;
use rspotify::model::{Id, IdError, PlayableItem, PlaylistId, PlaylistItem};
use rspotify::{ClientCredsSpotify, ClientError, ClientResult};
use serenity::async_trait;
use serenity::futures::StreamExt;
use std::sync::Arc;
use tokio::sync::Mutex;

pub(crate) struct FetchedTracks<'a> {
    youtube_searcher: &'a youtube::Searcher,
    items: Arc<Mutex<Paginator<'a, ClientResult<PlaylistItem>>>>,
}

impl<'a> FetchedTracks<'a> {
    pub(crate) fn new(
        youtube_searcher: &'a youtube::Searcher,
        items: Arc<Mutex<Paginator<'a, ClientResult<PlaylistItem>>>>,
    ) -> Self {
        Self {
            youtube_searcher,
            items,
        }
    }
}

#[async_trait]
impl AsyncIterator for FetchedTracks<'_> {
    type Item = anyhow::Result<Track>;

    async fn next(&mut self) -> Option<Self::Item> {
        let mut items = self.items.lock().await;

        loop {
            match items.next().await? {
                Err(error) => return Some(Err(error.into())),
                Ok(PlaylistItem {
                    track: Some(PlayableItem::Track(track)),
                    ..
                }) => match self.youtube_searcher.search(track.to_search_query()).await {
                    Err(error) => return Some(Err(error)),
                    Ok(None) => continue,
                    Ok(Some(track)) => return Some(Ok(track)),
                },
                _ => continue,
            };
        }
    }
}

pub(crate) struct Fetcher {
    spotify_client: Arc<ClientCredsSpotify>,
    youtube_searcher: Arc<youtube::Searcher>,
}

impl Fetcher {
    const ID_LENGTH: usize = 22;
    const URL_ID_PART: &'static str = "playlist/";

    pub(crate) fn new(
        spotify_client: Arc<ClientCredsSpotify>,
        youtube_searcher: Arc<youtube::Searcher>,
    ) -> Self {
        Self {
            spotify_client,
            youtube_searcher,
        }
    }
}

impl super::Fetcher for Fetcher {
    type Id<'a> = PlaylistId<'a>;

    fn id_length() -> usize {
        Self::ID_LENGTH
    }

    fn url_id_part() -> &'static str {
        Self::URL_ID_PART
    }

    fn create_id(id: &str) -> Result<Self::Id<'_>, IdError> {
        Self::Id::from_id(id)
    }
}

#[async_trait]
impl query::Fetcher for Fetcher {
    async fn fetch<'a>(&'a self, query: &'a str) -> anyhow::Result<Option<Fetched<'a>>> {
        let id = match <Self as super::Fetcher>::parse_id(query).ok() {
            None => return Ok(None),
            Some(id) => id,
        };

        let playlist = match self.spotify_client.playlist(id.clone(), None, None).await {
            Ok(playlist) => playlist,
            Err(error) => {
                // TODO: Use an if-let chain once they are stable.
                if let ClientError::Http(error) = &error {
                    if let HttpError::StatusCode(response) = error.as_ref() {
                        if response.status().as_u16() == 404 {
                            return Ok(None);
                        }
                    }
                }

                Err(error)?
            }
        };

        let playlist_items = self.spotify_client.playlist_items(id, None, None);

        Ok(Some(Fetched::new(
            playlist.name,
            playlist.id.url(),
            playlist
                .images
                .into_iter()
                .max_by_key(|image| image.width)
                .map(|image| image.url),
            Box::new(FetchedTracks::new(
                self.youtube_searcher.as_ref(),
                Arc::new(Mutex::new(playlist_items)),
            )),
        )))
    }
}
