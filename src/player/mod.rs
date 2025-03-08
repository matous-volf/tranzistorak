use std::sync::Arc;

use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use rand::SeedableRng;
use serenity::async_trait;
use serenity::client::Context;
use serenity::model::id::{ChannelId, GuildId};
use songbird::error::JoinError;
use songbird::events::EventHandler as VoiceEventHandler;
use songbird::input::YoutubeDl;
use songbird::tracks::TrackHandle;
use songbird::CoreEvent::DriverDisconnect;
use songbird::Event::{Core, Track};
use songbird::TrackEvent::End;
use songbird::{Call, Event, EventContext};
use tokio::sync::Mutex;

use crate::commands::CommandHandler;
use crate::youtube;
use crate::youtube::SearchResult;

const DISCONNECT_STOP_TIMEOUT_MS: u64 = 1000;

pub struct PlayerTrack {
    title: String,
    url: String,
    thumbnail_url: String,
}

impl PlayerTrack {
    pub fn new(title: String, url: String, thumbnail_url: String) -> PlayerTrack {
        PlayerTrack {
            title,
            url,
            thumbnail_url,
        }
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
    stopped: bool,
    rng: StdRng,
}

impl Player {
    pub async fn new(
        guild_id: GuildId,
        voice_channel_id: ChannelId,
        text_channel_id: ChannelId,
        context: Context,
    ) -> Result<Arc<Mutex<Player>>, JoinError> {
        let manager = songbird::get(&context).await.unwrap().clone();

        let driver = manager.join(guild_id, voice_channel_id).await?;

        driver.lock().await.deafen(true).await?;

        let player = Player {
            driver,
            audio: None,
            text_channel_id,
            context,
            queue: Vec::new(),
            current_playing_index: None,
            repeating: false,
            repeating_queue: false,
            stopped: false,
            rng: StdRng::from_os_rng(),
        };

        let player = Arc::new(Mutex::new(player));

        let player_clone = player.clone();
        let player_clone = player_clone.lock().await;
        let mut driver = player_clone.driver.lock().await;

        driver.add_global_event(Track(End), TrackEndHandler::new(player.clone()));
        driver.add_global_event(
            Core(DriverDisconnect),
            DriverDisconnectHandler::new(player.clone()),
        );

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

        let youtube_dl = YoutubeDl::new(reqwest::Client::new(), track.url.clone());

        let audio_handle = driver.play_only_input(youtube_dl.into());
        self.audio = Some(audio_handle);

        CommandHandler::track_started_playing(self, track, self.context.clone()).await;
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

        self.stopped = true;

        driver.stop();
        let _ = driver.leave().await;
        driver.remove_all_global_events();
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

    async fn disconnected(&mut self) {
        tokio::time::sleep(tokio::time::Duration::from_millis(
            DISCONNECT_STOP_TIMEOUT_MS,
        ))
        .await;

        if self.driver.lock().await.current_connection().is_none() {
            self.stop().await;
        }
    }

    pub async fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn current_playing_index(&self) -> Option<usize> {
        self.current_playing_index
    }

    pub async fn voice_channel_id(&self) -> songbird::id::ChannelId {
        self.driver
            .lock()
            .await
            .current_connection()
            .unwrap()
            .channel_id
            .unwrap()
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

struct DriverDisconnectHandler {
    player: Arc<Mutex<Player>>,
}

impl DriverDisconnectHandler {
    fn new(player: Arc<Mutex<Player>>) -> DriverDisconnectHandler {
        DriverDisconnectHandler { player }
    }
}

#[async_trait]
impl VoiceEventHandler for DriverDisconnectHandler {
    async fn act(&self, _: &EventContext<'_>) -> Option<Event> {
        self.player.lock().await.disconnected().await;
        None
    }
}
