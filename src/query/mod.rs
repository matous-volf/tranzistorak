use crate::model::Track;
use crate::utils::AsyncIterator;
use serenity::async_trait;

pub(crate) mod spotify;
pub(crate) mod youtube;

pub(crate) struct Fetched<'a> {
    pub(crate) title: String,
    pub(crate) url: String,
    pub(crate) thumbnail_url: Option<String>,
    pub(crate) tracks: Box<dyn AsyncIterator<Item = anyhow::Result<Track>> + 'a + Send + Sync>,
}

impl<'a> Fetched<'a> {
    pub(crate) fn new(
        title: String,
        url: String,
        thumbnail_url: Option<String>,
        tracks: Box<dyn AsyncIterator<Item = anyhow::Result<Track>> + 'a + Send + Sync>,
    ) -> Self {
        Self {
            title,
            url,
            thumbnail_url,
            tracks,
        }
    }
}

pub(crate) struct FetchedSingleTrack {
    track: Option<Track>,
}

impl FetchedSingleTrack {
    pub(crate) fn new(track: Option<Track>) -> Self {
        Self { track }
    }
}

#[async_trait]
impl AsyncIterator for FetchedSingleTrack {
    type Item = anyhow::Result<Track>;

    async fn next(&mut self) -> Option<Self::Item> {
        self.track.take().map(Ok)
    }
}

#[async_trait]
pub(crate) trait Fetcher {
    async fn fetch<'a>(&'a self, query: &'a str) -> anyhow::Result<Option<Fetched<'a>>>;
}
