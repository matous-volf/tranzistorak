use crate::query::spotify::ToSearchQuery;
use crate::query::{Fetched, FetchedSingleTrack};
use crate::{query, youtube};
use rspotify::clients::BaseClient;
use rspotify::http::HttpError;
use rspotify::model::{Id, IdError, TrackId};
use rspotify::{ClientCredsSpotify, ClientError};
use serenity::async_trait;
use std::sync::Arc;

pub(crate) struct Fetcher {
    spotify_client: Arc<ClientCredsSpotify>,
    youtube_searcher: Arc<youtube::Searcher>,
}

impl Fetcher {
    const ID_LENGTH: usize = 22;
    const URL_ID_PART: &'static str = "track/";

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
    type Id<'a> = TrackId<'a>;

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

        let spotify_track = match self.spotify_client.track(id.clone(), None).await {
            Ok(track) => track,
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

        let track = self
            .youtube_searcher
            .search(spotify_track.to_search_query())
            .await?;

        Ok(Some(Fetched::new(
            spotify_track.name,
            match spotify_track.id {
                None => return Ok(None),
                Some(id) => id.url(),
            },
            spotify_track
                .album
                .images
                .into_iter()
                .max_by_key(|image| image.width)
                .map(|image| image.url),
            Box::new(FetchedSingleTrack::new(track)),
        )))
    }
}
