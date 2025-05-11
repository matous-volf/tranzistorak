use crate::model::Track;
use crate::query;
use crate::query::Fetched;
use crate::utils::AsyncIterator;
use rustypipe::client::RustyPipe;
use rustypipe::model::{UrlTarget, VideoItem};
use rustypipe::param::{Country, Language};
use serenity::async_trait;
use std::vec::IntoIter;

pub(crate) struct FetchedTracks {
    items: IntoIter<VideoItem>,
}

impl FetchedTracks {
    pub(crate) fn new(items: Vec<VideoItem>) -> Self {
        Self {
            items: items.into_iter(),
        }
    }
}

#[async_trait]
impl AsyncIterator for FetchedTracks {
    type Item = anyhow::Result<Track>;

    async fn next(&mut self) -> Option<Self::Item> {
        self.items.next().map(Into::into).map(Ok)
    }
}

pub(crate) struct Fetcher {
    rusty_pipe_client: RustyPipe,
}

impl Fetcher {
    const PLAYLIST_ITEMS_FETCH_COUNT_LIMIT: usize = 1_000;
    const RESOLVE_YOUTUBE_MUSIC_ALBUM_IDS: bool = false;
    const RUSTY_PIPE_STORAGE_DIRECTORY_PATH: &'static str = "rusty_pipe_storage";

    pub(crate) fn new() -> Result<Self, rustypipe::error::Error> {
        Ok(Self {
            rusty_pipe_client: RustyPipe::builder()
                .country(Country::Cz)
                .lang(Language::Cs)
                .storage_dir(Self::RUSTY_PIPE_STORAGE_DIRECTORY_PATH)
                .build()?,
        })
    }
}

#[async_trait]
impl query::Fetcher for Fetcher {
    async fn fetch<'a>(&'a self, query: &'a str) -> anyhow::Result<Option<Fetched<'a>>> {
        let (id, url) = match self
            .rusty_pipe_client
            .query()
            .resolve_url(query, Self::RESOLVE_YOUTUBE_MUSIC_ALBUM_IDS)
            .await
        {
            Ok(url_target) => {
                let url = url_target.to_url();
                match url_target {
                    UrlTarget::Playlist { id } => (id, url),
                    _ => return Ok(None),
                }
            }
            // Includes URL parsing errors.
            Err(rustypipe::error::Error::Other(_)) => return Ok(None),
            Err(error) => Err(error)?,
        };

        let query = self.rusty_pipe_client.query();
        let mut playlist = match query.playlist(id).await {
            Ok(playlist) => playlist,
            Err(error) => {
                if let rustypipe::error::Error::Extraction(_) = error {
                    return Ok(None);
                }

                Err(error)?
            }
        };
        playlist
            .videos
            .extend_limit(query, Self::PLAYLIST_ITEMS_FETCH_COUNT_LIMIT)
            .await?;

        Ok(Some(Fetched::new(
            playlist.name,
            url,
            playlist
                .thumbnail
                .into_iter()
                .max_by_key(|thumbnail| thumbnail.width)
                .map(|thumbnail| thumbnail.url),
            Box::new(FetchedTracks::new(playlist.videos.items)),
        )))
    }
}

impl From<VideoItem> for Track {
    fn from(video_item: VideoItem) -> Self {
        Self::new(
            video_item.name,
            UrlTarget::Video {
                id: video_item.id,
                start_time: 0,
            }
            .to_url(),
            video_item
                .thumbnail
                .into_iter()
                .max_by_key(|thumbnail| thumbnail.width)
                .map(|thumbnail| thumbnail.url),
        )
    }
}
