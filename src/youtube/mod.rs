use std::sync::{Arc};
use invidious::ClientAsyncTrait;
use invidious::hidden::SearchItem;
use rspotify::ClientCredsSpotify;
use rspotify::clients::BaseClient;
use serenity::futures::StreamExt;
use tokio::sync::Mutex;
use crate::player::PlayerTrack;
const SPOTIFY_CLIENT_ID: &str = "10f3955d28e0454da9e4e0322b787e73";
const SPOTIFY_CLIENT_SECRET: &str = "1be82b3da0a54119b0e6f5fed63ad420";

const SPOTIFY_PLAYLIST_ID_LENGTH: usize = 22;

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
        let id_end = id_start + SPOTIFY_PLAYLIST_ID_LENGTH;
        return Some(url[id_start..id_end].to_string());
    }

    None
}

fn parse_spotify_track_id(url: &str) -> Option<String> {
    let id_start = url.find("track/").and_then(|i| Some(i + 6));

    if let Some(id_start) = id_start {
        let id_end = id_start + SPOTIFY_PLAYLIST_ID_LENGTH;
        return Some(url[id_start..id_end].to_string());
    }

    None
}

async fn get_spotify_playlist(id: &str) -> Option<Vec<PlayerTrack>> {
    let creds = rspotify::Credentials::new(
        SPOTIFY_CLIENT_ID,
        SPOTIFY_CLIENT_SECRET,
    );

    let spotify = ClientCredsSpotify::new(creds);
    spotify.request_token().await.unwrap();

    let id = rspotify::model::PlaylistId::from_id(id).unwrap();

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

    Some(tracks)
}

async fn get_spotify_track(id: &str) -> Option<PlayerTrack> {
    let creds = rspotify::Credentials::new(
        SPOTIFY_CLIENT_ID,
        SPOTIFY_CLIENT_SECRET,
    );

    let spotify = ClientCredsSpotify::new(creds);
    spotify.request_token().await.unwrap();

    let id = rspotify::model::TrackId::from_id(id).unwrap();
    let track = spotify.track(id).await;

    let track = match track {
        Ok(track) => track,
        Err(_) => return None,
    };

    get_youtube_video_from_spotify_track(&track).await
}

async fn get_youtube_playlist(id: &str) -> Option<Vec<PlayerTrack>> {
    let client = invidious::ClientAsync::default();
    let playlist = match client.playlist(id, None).await {
        Err(_) => return None,
        Ok(response) => response,
    };

    let mut tracks: Vec<PlayerTrack> = vec![];
    for video in playlist.videos {
        tracks.push(PlayerTrack::new(
            video.title.to_string(),
            format!("https://www.youtube.com/watch?v={}", video.id),
            video.thumbnails.first().and_then(|t| Some(t.url.clone())).unwrap_or("".to_string()),
        ));
    }

    if tracks.len() < 1 {
        return None;
    }

    return Some(tracks);
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
        if let SearchItem::Video { id, title, thumbnails, .. } = result {
            return Some(PlayerTrack::new(
                title.to_string(),
                format!("https://www.youtube.com/watch?v={}", id.to_string()),
                thumbnails.first().and_then(|t| Some(t.url.clone())).unwrap_or("".to_string()),
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

pub(crate) async fn get_tracks_from_query(query: &str) -> Option<Vec<PlayerTrack>> {
    let query = query;

    let id = parse_spotify_track_id(query);
    if let Some(id) = id {
        if let Some(track) = get_spotify_track(&id).await {
            return Some(vec![track]);
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

    Some(vec![get_youtube_video(query).await?])
}
