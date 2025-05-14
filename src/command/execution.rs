use crate::command::{Action, Command};
use crate::embed::EmbedIcon;
use crate::env::{SPOTIFY_API_CLIENT_ID, SPOTIFY_API_CLIENT_SECRET};
use crate::player::{Player, Track};
use crate::query::Fetcher;
use crate::{activity, embed, player, query, youtube};
use amplify_derive::Display;
use log::error;
use rspotify::ClientCredsSpotify;
use serenity::all::{ChannelId, Context, CreateMessage, GuildId};
use serenity::async_trait;
use songbird::error::JoinError;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::Mutex;
use unwrap_or_log::LogError;

#[derive(Error, Display, Debug)]
#[display(Debug)]
#[allow(dead_code)]
pub(crate) enum ExecutorCreationError {
    SpotifyClientTokenRetrieval(rspotify::ClientError),
    YouTubePlaylistFetcherCreation(rustypipe::error::Error),
}

#[derive(Error, Display, Debug)]
#[display(Debug)]
pub(crate) enum Error {
    UserCaused(UserCausedError),
    Internal(InternalError),
}

impl From<UserCausedError> for Error {
    fn from(user_caused_error: UserCausedError) -> Self {
        Self::UserCaused(user_caused_error)
    }
}

impl From<InternalError> for Error {
    fn from(internal_error: InternalError) -> Self {
        Self::Internal(internal_error)
    }
}

#[derive(Error, Display, Debug)]
#[display(Debug)]
pub(crate) enum UserCausedError {
    UserInDifferentVoiceChannel,
    CouldNotJoin(JoinError),
    NotPlaying,
    QueueMove(player::QueueMoveIndexExceedsQueueLengthError),
    Next(player::NextNoTrackError),
    Previous(player::PreviousNoTrackError),
}

#[derive(Error, Display, Debug)]
#[display(Debug)]
#[allow(dead_code)]
pub(crate) enum InternalError {
    PlayerCreation(player::CreationError),
    Play(anyhow::Error),
    Pause(songbird::error::ControlError),
    Resume(songbird::error::ControlError),
}

pub(crate) enum Executed<'a> {
    Play(Option<query::Fetched<'a>>),
    QueueView(player::Queue),
    QueueMove { index: usize },
    QueueRepeat(bool),
    QueueShuffle,
    Next,
    Previous,
    Pause,
    Resume,
    Repeat(bool),
    Stop,
}

impl From<player::CreationError> for InternalError {
    fn from(creation_error: player::CreationError) -> Self {
        Self::PlayerCreation(creation_error)
    }
}

type PlayerMap<S, V> = HashMap<GuildId, Arc<Mutex<Player<S, V>>>>;

pub(crate) struct Executor<V: player::VoiceTickCallback> {
    http_client: reqwest::Client,
    /* It is not needed to store these two in this struct, but this way they are noted as a part of
    the state. */
    #[allow(dead_code)]
    youtube_searcher: Arc<youtube::Searcher>,
    #[allow(dead_code)]
    spotify_client: Arc<ClientCredsSpotify>,
    query_fetchers: [Box<dyn Fetcher + Send + Sync>; 4],
    players: Mutex<PlayerMap<Arc<Self>, V>>,
    voice_tick_callback: Mutex<Option<V>>,
    activity_manager: Arc<activity::Manager>,
}

impl<V: player::VoiceTickCallback> Executor<V> {
    pub(crate) async fn new(
        on_voice_tick_callback: Option<V>,
        activity_manager: Arc<activity::Manager>,
    ) -> Result<Self, ExecutorCreationError> {
        let http_client = reqwest::Client::new();
        let youtube_searcher = Arc::new(youtube::Searcher::new(http_client.clone()));
        let spotify_client = Arc::new(ClientCredsSpotify::new(rspotify::Credentials::new(
            SPOTIFY_API_CLIENT_ID,
            SPOTIFY_API_CLIENT_SECRET,
        )));
        spotify_client
            .request_token()
            .await
            .map_err(ExecutorCreationError::SpotifyClientTokenRetrieval)?;

        Ok(Self {
            http_client,
            youtube_searcher: youtube_searcher.clone(),
            spotify_client: spotify_client.clone(),
            query_fetchers: [
                Box::new(query::spotify::playlist::Fetcher::new(
                    spotify_client.clone(),
                    youtube_searcher.clone(),
                )),
                Box::new(query::spotify::track::Fetcher::new(
                    spotify_client,
                    youtube_searcher.clone(),
                )),
                Box::new(
                    query::youtube::playlist::Fetcher::new()
                        .map_err(ExecutorCreationError::YouTubePlaylistFetcherCreation)?,
                ),
                Box::new(query::youtube::search::Fetcher::new(youtube_searcher)),
            ],
            players: Mutex::new(HashMap::new()),
            activity_manager,
            voice_tick_callback: Mutex::new(on_voice_tick_callback),
        })
    }

    pub(crate) async fn execute<'a>(
        self: &'a Arc<Self>,
        context: Context,
        command: &'a Command,
    ) -> Result<Executed<'a>, Error> {
        let player = {
            let player = self
                .players
                .lock()
                .await
                .get(&command.guild_id)
                .map(Clone::clone);

            let player_is_stopped = match &player {
                None => true,
                Some(player) => player.lock().await.is_stopped(),
            };

            match (player, player_is_stopped) {
                (Some(player), false) => {
                    match player.lock().await.voice_channel_id().await.log_error() {
                        Err(error) => {
                            error!("{error}");
                            player.lock().await.stop().await;
                            Err(UserCausedError::NotPlaying)?;
                        }
                        Ok(player_voice_channel_id) => {
                            if command.voice_channel_id.is_some_and(|voice_channel_id| {
                                player_voice_channel_id != voice_channel_id.into()
                            }) {
                                Err(UserCausedError::UserInDifferentVoiceChannel)?;
                            }
                        }
                    }
                    player
                }
                _ => match command.action {
                    Action::Play {
                        text_channel_id,
                        voice_channel_id,
                        ..
                    } => match self
                        .clone()
                        .create_player(command.guild_id, voice_channel_id, text_channel_id, context)
                        .await
                    {
                        Err(player::CreationError::ChannelJoin(error)) => {
                            let error = UserCausedError::CouldNotJoin(error);
                            error!("{error}");
                            Err(error)?
                        }
                        Err(error) => Err(InternalError::PlayerCreation(error))?,
                        Ok(new_player) => new_player,
                    },
                    _ => Err(UserCausedError::NotPlaying)?,
                },
            }
        };

        if let Some(text_channel_id) = command.text_channel_id {
            player.lock().await.set_text_channel_id(text_channel_id);
        }

        Ok(match &command.action {
            Action::Play { query, .. } | Action::VoicePlay { query } => Executed::Play(
                self.fetch_and_enqueue_query(&player, query)
                    .await
                    .map_err(InternalError::Play)?,
            ),
            Action::QueueView => Executed::QueueView(player.lock().await.queue().clone()),
            Action::QueueMove { index } => player
                .lock()
                .await
                .queue_move(*index)
                .await
                .map(|_| Executed::QueueMove { index: *index })
                .map_err(UserCausedError::QueueMove)?,
            Action::QueueRepeat(repeat) => {
                player.lock().await.queue_repeat(*repeat).await;
                Executed::QueueRepeat(*repeat)
            }
            Action::QueueShuffle => {
                player.lock().await.queue_shuffle().await;
                Executed::QueueShuffle
            }
            Action::Next => player
                .lock()
                .await
                .next()
                .await
                .map(|_| Executed::Next)
                .map_err(UserCausedError::Next)?,
            Action::Previous => player
                .lock()
                .await
                .previous()
                .await
                .map(|_| Executed::Previous)
                .map_err(UserCausedError::Previous)?,
            Action::Pause => player
                .lock()
                .await
                .pause()
                .await
                .map(|_| Executed::Pause)
                .map_err(InternalError::Pause)?,
            Action::Resume => player
                .lock()
                .await
                .resume()
                .await
                .map(|_| Executed::Resume)
                .map_err(InternalError::Resume)?,
            Action::Repeat(repeat) => {
                player.lock().await.repeat(*repeat).await;
                Executed::Repeat(*repeat)
            }
            Action::Stop => {
                player.lock().await.stop().await;
                Executed::Stop
            }
        })
    }

    async fn create_player(
        self: &Arc<Self>,
        guild_id: GuildId,
        voice_channel_id: ChannelId,
        text_channel_id: ChannelId,
        context: Context,
    ) -> Result<Arc<Mutex<Player<Arc<Self>, V>>>, player::CreationError> {
        let new_player = Player::new(
            self.http_client.clone(),
            guild_id,
            voice_channel_id,
            text_channel_id,
            context,
            Some(self.clone()),
            self.voice_tick_callback.lock().await.clone(),
        )
        .await?;
        self.players
            .lock()
            .await
            .insert(guild_id, new_player.clone());

        Ok(new_player)
    }

    async fn fetch_and_enqueue_query<'a>(
        self: &'a Arc<Self>,
        player: &Arc<Mutex<Player<Arc<Self>, V>>>,
        query: &'a str,
    ) -> anyhow::Result<Option<query::Fetched<'a>>> {
        for query_fetcher in self.query_fetchers.iter() {
            let mut fetched_query = match query_fetcher.fetch(query).await? {
                None => continue,
                Some(fetched_query) => fetched_query,
            };

            let mut has_fetched_any_tracks = false;
            while let Some(track) = fetched_query.tracks.next().await {
                match track {
                    Err(error) => error!("{error}"),
                    Ok(track) => {
                        player.lock().await.enqueue(track).await;
                        has_fetched_any_tracks = true;
                    }
                }
            }
            if !has_fetched_any_tracks {
                continue;
            }

            return Ok(Some(fetched_query));
        }

        Ok(None)
    }

    pub(crate) async fn player_text_channel_id(&self, guild_id: &GuildId) -> Option<ChannelId> {
        match self.players.lock().await.get(guild_id) {
            None => None,
            Some(player) => Some(player.lock().await.text_channel_id()),
        }
    }

    pub(crate) async fn set_voice_tick_callback(&self, voice_tick_callback: Option<V>) {
        *self.voice_tick_callback.lock().await = voice_tick_callback.clone();

        for player in self.players.lock().await.values() {
            player
                .lock()
                .await
                .set_voice_tick_callback(voice_tick_callback.clone());
        }
    }
}

#[async_trait]
impl<V: player::VoiceTickCallback> player::TrackStartedPlayingCallback for Arc<Executor<V>> {
    async fn on_started_playing(&self, track: Track, text_channel_id: ChannelId, context: Context) {
        _ = self
            .activity_manager
            .set_current_playing_track(track.clone())
            .await
            .log_error();

        let embed =
            embed::base("Přehrávání", EmbedIcon::YouTube, track.title).url(track.youtube_url);
        let embed = match track.thumbnail_url {
            None => embed,
            Some(thumbnail_url) => embed.thumbnail(thumbnail_url),
        };
        _ = text_channel_id
            .send_message(context.http, CreateMessage::new().embed(embed))
            .await
            .log_error();
    }
}
