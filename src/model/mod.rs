#[derive(Clone, Debug)]
pub(crate) struct Track {
    pub(crate) title: String,
    pub(crate) youtube_url: String,
    pub(crate) thumbnail_url: Option<String>,
}

impl Track {
    pub(crate) fn new(title: String, youtube_url: String, thumbnail_url: Option<String>) -> Self {
        Self {
            title,
            youtube_url,
            thumbnail_url,
        }
    }
}
