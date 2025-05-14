use crate::model::Track;
use songbird::input::{AudioStreamError, AuxMetadata, YoutubeDl};

const MAX_RESULTS: usize = 5;

pub(crate) struct Searcher {
    http_client: reqwest::Client,
}

impl Searcher {
    pub(crate) fn new(http_client: reqwest::Client) -> Self {
        Self { http_client }
    }

    pub(crate) async fn search(
        &self,
        query: impl AsRef<str>,
    ) -> Result<Option<Track>, anyhow::Error> {
        YoutubeDl::new_search(self.http_client.clone(), query.as_ref())
            .search(Some(MAX_RESULTS))
            .await
            .map(Some)
            .or_else(|error| match error {
                AudioStreamError::Fail(error) => {
                    if error.to_string().contains("no results found") {
                        Ok(None)
                    } else {
                        Err(anyhow::Error::from_boxed(error))
                    }
                }
                error => Err(error.into()),
            })
            .map(|results| {
                results.and_then(|results| results.map(Track::try_from).find_map(Result::ok))
            })
    }
}

impl TryFrom<AuxMetadata> for Track {
    type Error = ();

    fn try_from(aux_metadata: AuxMetadata) -> Result<Self, Self::Error> {
        Ok(Self::new(
            aux_metadata.title.ok_or(())?,
            aux_metadata.source_url.ok_or(())?,
            aux_metadata.thumbnail,
        ))
    }
}
