use std::env;
use std::sync::{Arc};
use invidious::ClientAsyncTrait;
use invidious::hidden::SearchItem;
use rspotify::ClientCredsSpotify;
use rspotify::clients::BaseClient;
use rspotify::model::Id;
use serenity::futures::StreamExt;
use tokio::sync::Mutex;
use crate::player::PlayerTrack;

const SPOTIFY_ID_LENGTH: usize = 22;

const YOUTUBE_VIDEO_BASE_URL: &str = "https://www.youtube.com/watch?v=";
const YOUTUBE_PLAYLIST_BASE_URL: &str = "https://www.youtube.com/playlist?list=";

pub struct SearchResult {
    tracks: Vec<PlayerTrack>,
    title: String,
    url: String,
    thumbnail_url: String,
}

impl SearchResult {
    pub fn new(tracks: Vec<PlayerTrack>, title: String, url: String, thumbnail_url: String) -> Self {
        Self {
            tracks,
            title,
            url,
            thumbnail_url,
        }
    }

    pub fn from_player_track(track: PlayerTrack) -> Self {
        Self {
            title: track.title().to_string(),
            url: track.url().to_string(),
            thumbnail_url: track.thumbnail_url().to_string(),
            tracks: vec![track],
        }
    }

    pub fn tracks(&self) -> &Vec<PlayerTrack> { &self.tracks }
    pub fn title(&self) -> &str { &self.title }
    pub fn url(&self) -> &str { &self.url }
    pub fn thumbnail_url(&self) -> &str { &self.thumbnail_url }
}

fn parse_youtube_playlist_id(url: &str) -> Option<String> {
    let id_start = url.find('=').and_then(|i| Some(i + 1));

    if let Some(id_start) = id_start {
        let id_end = url.find('&').unwrap_or(url.len());
        return Some(url[id_start..id_end].to_string());
    }

    return None;
}

fn parse_spotify_playlist_id(url: &str) -> Option<String> {
    let id_start = url.find("playlist/").and_then(|i| Some(i + 9));

    if let Some(id_start) = id_start {
        let id_end = id_start + SPOTIFY_ID_LENGTH;
        return Some(url[id_start..id_end].to_string());
    }

    None
}

fn parse_spotify_track_id(url: &str) -> Option<String> {
    let id_start = url.find("track/").and_then(|i| Some(i + 6));

    if let Some(id_start) = id_start {
        let id_end = id_start + SPOTIFY_ID_LENGTH;
        return Some(url[id_start..id_end].to_string());
    }

    None
}

async fn get_spotify_playlist(id: &str) -> Option<SearchResult> {
    let creds = rspotify::Credentials::new(
        &env::var("SPOTIFY_CLIENT_ID").unwrap(),
        &env::var("SPOTIFY_CLIENT_SECRET").unwrap(),
    );

    let spotify = ClientCredsSpotify::new(creds);
    spotify.request_token().await.unwrap();

    let id = rspotify::model::PlaylistId::from_id(id).unwrap();

    let playlist = match spotify.playlist(id.clone(), None, None).await {
        Ok(playlist) => playlist,
        Err(_) => return None,
    };

    let playlist_items = spotify.playlist_items(id.clone(), None, None);

    let tracks = Arc::new(Mutex::new(vec![]));
    playlist_items
        .for_each_concurrent(10, |item| async {
            let item = match item {
                Ok(item) => item,
                Err(_) => return,
            };

            let item = match item.track {
                Some(item) => item,
                None => return,
            };

            let item = match item {
                rspotify::model::PlayableItem::Track(item) => item,
                _ => return,
            };

            if let Some(track) = get_youtube_video_from_spotify_track(&item).await {
                tracks.clone().lock().await.push(track);
            }
        })
        .await;

    let tracks = Arc::try_unwrap(tracks).ok().unwrap().into_inner();

    if tracks.len() < 1 {
        return None;
    }

    Some(
        SearchResult::new(
            tracks,
            format!("{}", playlist.name),
            playlist.id.url(),
            playlist.images.first().and_then(|image| Some(image.url.clone())).unwrap_or("".to_string()),
        )
    )
}

async fn get_spotify_track(id: &str) -> Option<PlayerTrack> {
    let creds = rspotify::Credentials::new(
        &env::var("SPOTIFY_CLIENT_ID").unwrap(),
        &env::var("SPOTIFY_CLIENT_SECRET").unwrap(),
    );

    let spotify = ClientCredsSpotify::new(creds);
    spotify.request_token().await.unwrap();

    let id = rspotify::model::TrackId::from_id(id).unwrap();
    let track = spotify.track(id, None).await;

    let track = match track {
        Ok(track) => track,
        Err(_) => return None,
    };

    get_youtube_video_from_spotify_track(&track).await
}

async fn get_youtube_playlist(id: &str) -> Option<SearchResult> {
    let client = invidious::ClientAsync::default();
    let playlist = match client.playlist(id, None).await {
        Err(_) => return None,
        Ok(response) => response,
    };

    let mut tracks: Vec<PlayerTrack> = vec![];
    for video in playlist.videos {
        tracks.push(PlayerTrack::new(
            video.title.to_string(),
            format!("{}{}", YOUTUBE_VIDEO_BASE_URL, video.id),
            video.thumbnails.first().and_then(|t| Some(t.url.clone())).unwrap_or("".to_string()),
        ));
    }

    if tracks.len() < 1 {
        return None;
    }

    Some(SearchResult::new(
        tracks,
        playlist.title,
        format!("{}{}", YOUTUBE_PLAYLIST_BASE_URL, playlist.id),
        playlist.thumbnail,
    ))
}

async fn get_youtube_video(query: &str) -> Option<PlayerTrack> {
    let client = invidious::ClientAsync::default();

    let results = match client.search(Some(format!("q={}", query).as_str())).await {
        Ok(results) => results.items,
        Err(_) => return None,
    };

    if results.len() < 1 {
        return None;
    }

    for result in results.iter() {
        if let SearchItem::Video(video) = result {
            let thumbnail_url = video.thumbnails.first().and_then(|t| Some(t.url.clone())).unwrap_or("".to_string());

            return Some(PlayerTrack::new(
                video.title.to_string(),
                format!("{}{}", YOUTUBE_VIDEO_BASE_URL, video.id.to_string()),
                thumbnail_url.clone(),
            ));
        }
    }

    return None;
}

async fn get_youtube_video_from_spotify_track(track: &rspotify::model::track::FullTrack) -> Option<PlayerTrack> {
    let artist = track.artists.first().and_then(|artist| Some(artist.name.as_str())).unwrap_or("");
    let query = format!("{} {}", artist, track.name);

    get_youtube_video(query.as_str()).await
}

pub(crate) async fn get_tracks_from_query(query: &str) -> Option<SearchResult> {
    let query = query;

    let id = parse_spotify_track_id(query);
    if let Some(id) = id {
        if let Some(track) = get_spotify_track(&id).await {
            return Some(SearchResult::from_player_track(track));
        }
    }

    let id = parse_spotify_playlist_id(query);
    if let Some(id) = id {
        if let Some(tracks) = get_spotify_playlist(&id).await {
            return Some(tracks);
        }
    }

    let id = parse_youtube_playlist_id(query);
    if let Some(id) = id {
        if let Some(tracks) = get_youtube_playlist(&id).await {
            return Some(tracks);
        }
    }

    Some(SearchResult::from_player_track(get_youtube_video(query).await?))
}
