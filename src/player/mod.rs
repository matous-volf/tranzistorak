use songbird::error::JoinError;
use songbird::Event::Track;
use songbird::TrackEvent::End;
use songbird::tracks::TrackHandle;
use std::sync::Arc;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rand::seq::SliceRandom;
use serenity::async_trait;
use serenity::client::{Context};
use serenity::model::id::{ChannelId, GuildId};
use songbird::{Call, Event, EventContext};
use songbird::events::EventHandler as VoiceEventHandler;
use tokio::sync::Mutex;

use crate::youtube;
use crate::commands::CommandHandler;
use crate::youtube::SearchResult;

pub struct PlayerTrack {
    title: String,
    url: String,
    thumbnail_url: String,
}

impl PlayerTrack {
    pub fn new(title: String, url: String, thumbnail_url: String) -> PlayerTrack {
        PlayerTrack { title, url, thumbnail_url }
    }

    pub fn title(&self) -> &str {
        &self.title
    }
    pub fn url(&self) -> &str {
        &self.url
    }
    pub fn thumbnail_url(&self) -> &str {
        &self.thumbnail_url
    }
}

impl Clone for PlayerTrack {
    fn clone(&self) -> Self {
        Self {
            title: self.title().to_string(),
            url: self.url().to_string(),
            thumbnail_url: self.thumbnail_url().to_string(),
        }
    }
}

pub struct Player {
    driver: Arc<Mutex<Call>>,
    audio: Option<TrackHandle>,
    text_channel_id: ChannelId,
    context: Context,
    queue: Vec<PlayerTrack>,
    current_playing_index: Option<usize>,
    repeating: bool,
    repeating_queue: bool,
    rng: StdRng,
}

impl Player {
    pub async fn new(
        guild_id: GuildId,
        voice_channel_id: ChannelId,
        text_channel_id: ChannelId,
        context: &Context)
        -> Result<Arc<Mutex<Player>>, JoinError> {
        let manager = songbird::get(&context).await.unwrap().clone();

        let (driver, join_result) = manager.join(guild_id, voice_channel_id).await;
        join_result?;

        let _ = driver.lock().await.deafen(true).await;

        let player = Player {
            driver,
            audio: None,
            text_channel_id,
            context: context.clone(),
            queue: Vec::new(),
            current_playing_index: None,
            repeating: false,
            repeating_queue: false,
            rng: StdRng::from_entropy(),
        };

        let player = Arc::new(Mutex::new(player));

        player.clone().lock().await
            .driver.lock().await
            .add_global_event(Track(End), TrackEndHandler::new(player.clone()));

        Ok(player)
    }

    pub async fn enqueue(&mut self, query: &str) -> Option<SearchResult> {
        let search_result = youtube::get_tracks_from_query(query).await?;

        for track in search_result.tracks() {
            self.queue.push(track.clone());
        }

        if self.current_playing_index.is_none() {
            let next_playing_index = self.queue.len() - search_result.tracks().len();
            self.play(next_playing_index).await;
        }

        Some(search_result)
    }

    async fn play(&mut self, index: usize) {
        let mut driver = self.driver.lock().await;
        driver.stop();

        self.current_playing_index = Some(index);
        let track = &self.queue[index];

        let source = match songbird::ytdl(&track.url).await {
            Ok(source) => source,
            Err(_) => return
        };

        let (audio, audio_handle) = songbird::create_player(source);
        self.audio = Some(audio_handle);

        driver.play_only(audio);
        CommandHandler::track_started_playing(&self, track, self.context.clone()).await;
    }

    pub async fn next(&mut self) -> Result<(), ()> {
        let current_playing_index = match self.current_playing_index {
            None => return Err(()),
            Some(index) => {
                if index + 1 >= self.queue.len() {
                    return Err(());
                }
                index
            }
        };

        self.play(current_playing_index + 1).await;
        Ok(())
    }

    pub async fn previous(&mut self) -> Result<(), ()> {
        let current_playing_index = match self.current_playing_index {
            None => {
                self.play(self.queue.len() - 1).await;
                return Ok(());
            }
            Some(index) => {
                if index < 1 {
                    return Err(());
                }

                index
            }
        };

        self.play(current_playing_index - 1).await;
        Ok(())
    }

    pub async fn queue(&self) -> &Vec<PlayerTrack> {
        &self.queue
    }

    pub async fn queue_move(&mut self, index: usize) -> Result<(), ()> {
        if index >= self.queue.len() {
            return Err(());
        }

        self.play(index).await;

        Ok(())
    }

    pub async fn queue_repeat(&mut self, repeat: bool) {
        self.repeating_queue = repeat;
    }

    pub async fn queue_shuffle(&mut self) {
        self.queue.shuffle(&mut self.rng);
        self.play(0).await;
    }

    pub async fn pause(&self) {
        let audio = self.audio.as_ref().unwrap();
        if audio.get_info().await.is_err() {
            return;
        }
        audio.pause().unwrap();
    }

    pub async fn resume(&self) {
        let audio = self.audio.as_ref().unwrap();
        if audio.get_info().await.is_err() {
            return;
        }
        audio.play().unwrap();
    }

    pub async fn repeat(&mut self, repeat: bool) {
        self.repeating = repeat;
    }

    pub async fn stop(&mut self) {
        let mut driver = self.driver.lock().await;

        driver.stop();
        let _ = driver.leave().await;
    }

    async fn track_ended(&mut self) {
        if self.repeating {
            self.play(self.current_playing_index.unwrap()).await;
            return;
        }

        if self.next().await.is_err() {
            if self.repeating_queue {
                self.play(0).await;
            } else {
                self.current_playing_index = None;
            }
        };
    }

    pub async fn is_stopped(&self) -> bool {
        self.driver.lock().await.current_connection().is_none()
    }

    pub fn current_playing_index(&self) -> Option<usize> {
        self.current_playing_index
    }

    pub async fn voice_channel_id(&self) -> songbird::id::ChannelId {
        self.driver.lock().await.current_connection().unwrap().channel_id.unwrap()
    }

    pub fn text_channel_id(&self) -> ChannelId {
        self.text_channel_id
    }

    pub fn set_text_channel_id(&mut self, channel_id: ChannelId) {
        self.text_channel_id = channel_id
    }
}

struct TrackEndHandler {
    player: Arc<Mutex<Player>>,
}

impl TrackEndHandler {
    fn new(player: Arc<Mutex<Player>>) -> TrackEndHandler {
        TrackEndHandler { player }
    }
}

#[async_trait]
impl VoiceEventHandler for TrackEndHandler {
    async fn act(&self, _: &EventContext<'_>) -> Option<Event> {
        self.player.lock().await.track_ended().await;
        None
    }
}
