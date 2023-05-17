use crate::player::PlayerTrack;
use yt_api;

const YOUTUBE_API_KEY: &str = "";
const YOUTUBE_PLAYLIST_ID_LENGTH: usize = 34;
const YOUTUBE_VIDEO_ID_LENGTH: usize = 11;

pub(crate) async fn get_tracks_from_query(query: &str) -> Option<Vec<PlayerTrack>> {
    let api_key = yt_api::ApiKey::new(YOUTUBE_API_KEY);

    let mut query = query;

    let id_start = query.find('=').and_then(|i| Some(i + 1));

    match id_start {
        Some(id_start) => {
            if id_start <= query.len() - YOUTUBE_PLAYLIST_ID_LENGTH {
                let playlist_id = query[id_start..id_start + YOUTUBE_PLAYLIST_ID_LENGTH].to_string();

                let playlist = yt_api::playlistitems::PlaylistItems::new(api_key.clone())
                    .playlist_id(playlist_id)
                    .max_results(255)
                    .await;

                match playlist {
                    Ok(response) => {
                        let mut tracks: Vec<PlayerTrack> = vec![];
                        for playlist_item in response.items {
                            let thumbnails = playlist_item.snippet.thumbnails.as_ref().unwrap();

                            let mut thumbnail: Option<&yt_api::playlistitems::Thumbnail> = None;

                            let thumbnails = vec![
                                &thumbnails.medium,
                                &thumbnails.maxres,
                                &thumbnails.high,
                                &thumbnails.standard,
                                &thumbnails.default,
                            ];

                            for t in thumbnails {
                                if let Some(t) = t {
                                    thumbnail = Some(t);
                                    break;
                                }
                            }

                            if thumbnail.is_none() {
                                // the video is private
                                continue;
                            }

                            tracks.push(PlayerTrack::new(
                                playlist_item.snippet.title.as_ref().unwrap().to_string(),
                                format!("https://www.youtube.com/watch?v={}", playlist_item.snippet.resource_id.video_id),
                                thumbnail.unwrap().url.to_string(),
                            ));
                        }

                        return Some(tracks);
                    }
                    Err(_) => {}
                }
            }
        }
        None => {}
    }

    if let Some(id_start) = id_start {
        if query.len() > id_start + YOUTUBE_VIDEO_ID_LENGTH {
            query = &query[0..id_start + YOUTUBE_VIDEO_ID_LENGTH];
        }
    }

    let response = yt_api::search::SearchList::new(api_key)
        .q(query)
        .item_type(yt_api::search::ItemType::Video)
        .await;

    let results = match response {
        Err(_) => return None,
        Ok(response) => response.items
    };

    if results.len() < 1 {
        return None;
    }

    let video = results.first().unwrap();

    Some(vec![PlayerTrack::new(
        video.snippet.title.as_ref().unwrap().to_string(),
        format!("https://www.youtube.com/watch?v={}", video.id.video_id.as_ref().unwrap().to_string()),
        video.snippet.thumbnails.as_ref().unwrap().medium.as_ref().unwrap().url.to_string(),
    )])
}
