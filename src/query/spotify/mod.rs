use rspotify::model::IdError;

pub(crate) mod playlist;
pub(crate) mod track;

trait Fetcher {
    type Id<'a>;

    fn id_length() -> usize;
    fn url_id_part() -> &'static str;

    fn create_id(id: &str) -> Result<Self::Id<'_>, IdError>;

    fn parse_id(url: &str) -> Result<Self::Id<'_>, ()> {
        url.find(Self::url_id_part())
            .map(|index| index + Self::url_id_part().len())
            .and_then(|start_index| url.get(start_index..start_index + Self::id_length()))
            .and_then(|id| Self::create_id(id).ok())
            .ok_or(())
    }
}

trait ToSearchQuery {
    fn title(&self) -> impl AsRef<str>;
    fn artist_name(&self) -> Option<impl AsRef<str>>;

    fn to_search_query(&self) -> String {
        self.artist_name()
            .map(|artist_name| format!("{} {}", artist_name.as_ref(), self.title().as_ref()))
            .unwrap_or(self.title().as_ref().to_owned())
    }
}

impl ToSearchQuery for rspotify::model::FullTrack {
    fn title(&self) -> impl AsRef<str> {
        self.name.as_str()
    }

    fn artist_name(&self) -> Option<impl AsRef<str>> {
        self.artists.first().map(|artist| artist.name.as_str())
    }
}
