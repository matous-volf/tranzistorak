use crate::query::{Fetched, FetchedSingleTrack};
use crate::{query, youtube};
use serenity::async_trait;
use std::sync::Arc;

pub(crate) struct Fetcher {
    youtube_searcher: Arc<youtube::Searcher>,
}

impl Fetcher {
    pub(crate) fn new(youtube_searcher: Arc<youtube::Searcher>) -> Self {
        Self { youtube_searcher }
    }
}

#[async_trait]
impl query::Fetcher for Fetcher {
    async fn fetch<'a>(&'a self, query: &'a str) -> anyhow::Result<Option<Fetched<'a>>> {
        self.youtube_searcher.search(query).await.map(|track| {
            track.map(|track| {
                Fetched::new(
                    track.title.clone(),
                    track.youtube_url.clone(),
                    track.thumbnail_url.clone(),
                    Box::new(FetchedSingleTrack::new(Some(track))),
                )
            })
        })
    }
}
