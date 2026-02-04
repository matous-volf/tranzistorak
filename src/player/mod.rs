pub(crate) use crate::model::Track;
use amplify_derive::Display;
use rand::SeedableRng;
use rand::rngs::StdRng;
use rand::seq::SliceRandom;
use serenity::all::{ChannelId, Context, GuildId};
use serenity::async_trait;
use songbird::error::JoinError;
use songbird::events::context_data::VoiceTick;
use songbird::input::YoutubeDl;
use songbird::tracks::{PlayMode, TrackHandle};
use songbird::{Call, CoreEvent, Event, EventContext, EventHandler, TrackEvent};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::time::Duration;
use unwrap_or_log::LogError;

const DISCONNECT_STOP_TIMEOUT_DURATION: Duration = Duration::from_secs(1);

#[derive(Error, Display, Debug)]
#[display(Debug)]
pub(crate) enum CreationError {
    SongbirdClientRetrieval,
    ChannelJoin(JoinError),
}

#[derive(Error, Display, Debug)]
#[display(Debug)]
pub(crate) struct NextNoTrackError;

#[derive(Error, Display, Debug)]
#[display(Debug)]
pub(crate) struct PreviousNoTrackError;

#[derive(Error, Display, Debug)]
#[display(Debug)]
struct OnTrackEndedNotPlayingError;

#[derive(Error, Display, Debug)]
#[display(Debug)]
pub(crate) struct QueueMoveIndexExceedsQueueLengthError(pub(crate) usize);

#[derive(Error, Display, Debug)]
#[display(Debug)]
pub(crate) struct NoVoiceChannelIdError;

impl From<JoinError> for CreationError {
    fn from(join_error: JoinError) -> Self {
        Self::ChannelJoin(join_error)
    }
}

#[derive(Default, Clone)]
pub(crate) struct Queue {
    pub(crate) tracks: Vec<Track>,
    pub(crate) current_playing_track_index: Option<usize>,
}

#[async_trait]
pub(crate) trait TrackStartedPlayingCallback: Send + Sync + Clone + 'static {
    async fn on_started_playing(&self, track: Track, channel_id: ChannelId, context: Context);
}

#[async_trait]
pub(crate) trait VoiceTickCallback: Send + Sync + Clone + 'static {
    async fn on_voice_tick(&self, guild_id: GuildId, voice_tick: VoiceTick);
}

pub(crate) struct Player<S: TrackStartedPlayingCallback, V: VoiceTickCallback> {
    http_client: reqwest::Client,
    voice_driver: Arc<Mutex<Call>>,
    track_handle: Option<TrackHandle>,
    guild_id: GuildId,
    text_channel_id: ChannelId,
    context: Context,
    queue: Queue,
    repeating: bool,
    repeating_queue: bool,
    is_stopped: bool,
    // TODO: Make the callbacks accept references instead.
    track_started_playing_callback: Option<S>,
    voice_tick_callback: Option<V>,
    rng: StdRng,
}

impl<S: TrackStartedPlayingCallback, V: VoiceTickCallback> Player<S, V> {
    pub(crate) async fn new(
        http_client: reqwest::Client,
        guild_id: GuildId,
        voice_channel_id: ChannelId,
        text_channel_id: ChannelId,
        context: Context,
        track_started_playing_callback: Option<S>,
        voice_tick_callback: Option<V>,
    ) -> Result<Arc<Mutex<Self>>, CreationError> {
        let manager = songbird::get(&context)
            .await
            .ok_or(CreationError::SongbirdClientRetrieval)?
            .clone();

        let voice_driver = manager.join(guild_id, voice_channel_id).await?;
        voice_driver.lock().await.deafen(true).await?;

        let player = Arc::new(Mutex::new(Self {
            http_client,
            voice_driver,
            track_handle: None,
            guild_id,
            text_channel_id,
            context,
            queue: Queue::default(),
            repeating: false,
            repeating_queue: false,
            is_stopped: false,
            track_started_playing_callback,
            voice_tick_callback,
            rng: StdRng::from_os_rng(),
        }));

        let player_clone = player.clone();
        let player_clone = player_clone.lock().await;
        let mut voice_driver = player_clone.voice_driver.lock().await;

        let voice_driver_event_handler = VoiceDriverEventHandler::new(player.clone());
        voice_driver.add_global_event(TrackEvent::End.into(), voice_driver_event_handler.clone());
        voice_driver.add_global_event(
            CoreEvent::DriverDisconnect.into(),
            voice_driver_event_handler.clone(),
        );
        voice_driver.add_global_event(CoreEvent::VoiceTick.into(), voice_driver_event_handler);

        Ok(player)
    }

    pub(crate) async fn enqueue(&mut self, track: Track) {
        self.queue.tracks.push(track.clone());

        if self.queue.current_playing_track_index.is_none() {
            self.play(self.queue.tracks.len() - 1).await;
        }
    }

    async fn play(&mut self, track_index: usize) {
        let mut driver = self.voice_driver.lock().await;
        driver.stop();

        self.queue.current_playing_track_index = Some(track_index);
        let track = &self.queue.tracks[track_index];

        let youtube_dl = YoutubeDl::new(self.http_client.clone(), track.youtube_url.clone());
        self.track_handle = Some(driver.play_only_input(youtube_dl.into()));

        if let Some(track_started_playing_callback) = self.track_started_playing_callback.as_ref() {
            track_started_playing_callback
                .on_started_playing(track.clone(), self.text_channel_id, self.context.clone())
                .await;
        }
    }

    pub(crate) async fn next(&mut self) -> Result<(), NextNoTrackError> {
        let current_playing_track_index = match self.queue.current_playing_track_index {
            None => Err(NextNoTrackError)?,
            Some(index) => {
                if index + 1 >= self.queue.tracks.len() {
                    Err(NextNoTrackError)?;
                }
                index
            }
        };

        self.play(current_playing_track_index + 1).await;
        Ok(())
    }

    pub(crate) async fn previous(&mut self) -> Result<(), PreviousNoTrackError> {
        let track_index = match self.queue.current_playing_track_index {
            None => self.queue.tracks.len(),
            Some(index) => index,
        }
        .checked_sub(1)
        .ok_or(PreviousNoTrackError)?;

        self.play(track_index).await;
        Ok(())
    }

    pub(crate) fn queue(&self) -> &Queue {
        &self.queue
    }

    pub(crate) async fn queue_move(
        &mut self,
        index: usize,
    ) -> Result<(), QueueMoveIndexExceedsQueueLengthError> {
        if index >= self.queue.tracks.len() {
            Err(QueueMoveIndexExceedsQueueLengthError(index))?;
        }

        self.play(index).await;

        Ok(())
    }

    pub(crate) async fn queue_repeat(&mut self, repeat: bool) {
        self.repeating_queue = repeat;
    }

    pub(crate) async fn queue_shuffle(&mut self) {
        self.queue.tracks.shuffle(&mut self.rng);
        self.play(0).await;
    }

    pub(crate) async fn pause(&self) -> songbird::error::TrackResult<()> {
        if let Some(track_handle) = &self.track_handle {
            return track_handle.pause();
        }
        Ok(())
    }

    pub(crate) async fn resume(&self) -> songbird::error::TrackResult<()> {
        if let Some(track_handle) = &self.track_handle {
            return track_handle.play();
        }
        Ok(())
    }

    pub(crate) async fn repeat(&mut self, repeat: bool) {
        self.repeating = repeat;
    }

    pub(crate) async fn stop(&mut self) {
        self.is_stopped = true;

        let mut voice_driver = self.voice_driver.lock().await;
        voice_driver.stop();
        _ = voice_driver.leave().await;
        voice_driver.remove_all_global_events();
    }

    async fn on_track_ended(&mut self) -> Result<(), OnTrackEndedNotPlayingError> {
        if self.repeating {
            self.play(
                self.queue
                    .current_playing_track_index
                    .ok_or(OnTrackEndedNotPlayingError)?,
            )
            .await;
            return Ok(());
        }

        if let Err(NextNoTrackError) = self.next().await {
            if self.repeating_queue {
                self.play(0).await;
            } else {
                self.queue.current_playing_track_index = None;
            }
        }

        Ok(())
    }

    async fn on_disconnected(&mut self) {
        tokio::time::sleep(DISCONNECT_STOP_TIMEOUT_DURATION).await;

        if self
            .voice_driver
            .lock()
            .await
            .current_connection()
            .is_none()
        {
            self.stop().await;
        }
    }

    async fn on_voice_tick(&self, voice_tick: VoiceTick) {
        if let Some(voice_tick_callback) = self.voice_tick_callback.as_ref() {
            voice_tick_callback
                .on_voice_tick(self.guild_id, voice_tick)
                .await;
        }
    }

    pub(crate) fn set_voice_tick_callback(&mut self, voice_tick_callback: Option<V>) {
        self.voice_tick_callback = voice_tick_callback
    }

    pub(crate) async fn voice_channel_id(
        &self,
    ) -> Result<songbird::id::ChannelId, NoVoiceChannelIdError> {
        self.voice_driver
            .lock()
            .await
            .current_connection()
            .ok_or(NoVoiceChannelIdError)?
            .channel_id
            .ok_or(NoVoiceChannelIdError)
    }

    pub(crate) fn text_channel_id(&self) -> ChannelId {
        self.text_channel_id
    }

    pub(crate) fn set_text_channel_id(&mut self, channel_id: ChannelId) {
        self.text_channel_id = channel_id
    }

    pub(crate) fn is_stopped(&self) -> bool {
        self.is_stopped
    }
}

#[derive(Clone)]
struct VoiceDriverEventHandler<S: TrackStartedPlayingCallback, V: VoiceTickCallback> {
    player: Arc<Mutex<Player<S, V>>>,
}

impl<S: TrackStartedPlayingCallback, V: VoiceTickCallback> VoiceDriverEventHandler<S, V> {
    pub(crate) fn new(player: Arc<Mutex<Player<S, V>>>) -> Self {
        Self { player }
    }
}

#[async_trait]
impl<S: TrackStartedPlayingCallback, V: VoiceTickCallback> EventHandler
    for VoiceDriverEventHandler<S, V>
{
    async fn act(&self, context: &EventContext<'_>) -> Option<Event> {
        match context {
            EventContext::Track([(track_state, _)]) => {
                if let PlayMode::End = track_state.playing {
                    let mut player = self.player.lock().await;
                    if player.on_track_ended().await.log_error().is_err() {
                        player.stop().await
                    }
                }
            }
            EventContext::VoiceTick(voice_tick) => {
                self.player
                    .lock()
                    .await
                    .on_voice_tick(voice_tick.clone())
                    .await;
            }
            EventContext::DriverDisconnect(_) => self.player.lock().await.on_disconnected().await,
            _ => (),
        }
        None
    }
}
