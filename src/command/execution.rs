use crate::embed::EmbedIcon;
use crate::player::{Player, Track};
use crate::query::Fetcher;
use crate::{activity, embed, player, query, youtube};
use amplify_derive::Display;
use log::error;
use rspotify::ClientCredsSpotify;
use serenity::all::{
    ChannelId, CommandDataOptionValue, CommandInteraction, Context, CreateMessage, GuildId,
};
use songbird::error::JoinError;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{Mutex, RwLock};
use unwrap_or_log::LogError;

#[derive(Error, Display, Debug)]
#[display(Debug)]
#[allow(dead_code)]
pub(crate) enum ExecutorCreationError {
    SpotifyClientTokenRetrieval(rspotify::ClientError),
    YouTubePlaylistFetcherCreation(rustypipe::error::Error),
}

/// Error caused by the user invoking a command.
#[derive(Error, Display, Debug)]
#[display(Debug)]
#[allow(dead_code)]
pub(crate) enum Error {
    NotInGuild,
    UserNotInVoiceChannel,
    UserInDifferentVoiceChannel,
    CouldNotJoin(JoinError),
    NotPlaying,
    QueueMove(player::QueueMoveIndexExceedsQueueLengthError),
    Next(player::NextNoTrackError),
    Previous(player::PreviousNoTrackError),
}

pub(crate) enum Executed<'a> {
    Play(Option<query::Fetched<'a>>),
    QueueView(player::Queue),
    QueueMove(usize),
    QueueRepeat(bool),
    QueueShuffle,
    Next,
    Previous,
    Pause,
    Resume,
    Repeat(bool),
    Stop,
}

/// Error not caused by the user invoking a command.
#[derive(Error, Display, Debug)]
#[display(Debug)]
#[allow(dead_code)]
pub(crate) enum ExecutionError {
    GuildNotFoundById,
    PlayerCreation(player::CreationError),
    InvalidOption,
    Play(anyhow::Error),
    Pause(songbird::error::ControlError),
    Resume(songbird::error::ControlError),
}

impl From<player::CreationError> for ExecutionError {
    fn from(creation_error: player::CreationError) -> Self {
        Self::PlayerCreation(creation_error)
    }
}

pub(crate) struct Executor {
    http_client: reqwest::Client,
    /* It is not needed to store these two in this struct, but this way they are noted as a part of
    the state. */
    #[allow(dead_code)]
    youtube_searcher: Arc<youtube::Searcher>,
    #[allow(dead_code)]
    spotify_client: Arc<ClientCredsSpotify>,
    query_fetchers: [Box<dyn Fetcher + Send + Sync>; 4],
    players: Mutex<HashMap<GuildId, Arc<Mutex<Player>>>>,
    activity_manager: Arc<RwLock<activity::Manager>>,
}

impl Executor {
    pub(crate) async fn new(
        spotify_api_credentials: rspotify::Credentials,
        activity_manager: Arc<RwLock<activity::Manager>>,
    ) -> Result<Self, ExecutorCreationError> {
        let http_client = reqwest::Client::new();
        let youtube_searcher = Arc::new(youtube::Searcher::new(http_client.clone()));
        let spotify_client = Arc::new(ClientCredsSpotify::new(spotify_api_credentials));
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
        })
    }

    pub(crate) async fn execute<'a>(
        &'a self,
        context: Context,
        command: &'a CommandInteraction,
    ) -> Result<Result<Executed<'a>, Error>, ExecutionError> {
        let guild = match command.guild_id {
            None => return Ok(Err(Error::NotInGuild)),
            Some(guild_id) => guild_id
                .to_guild_cached(context.cache.as_ref())
                .ok_or(ExecutionError::GuildNotFoundById)?
                .clone(),
        };

        let voice_channel_id = match guild
            .voice_states
            .get(&command.user.id)
            .and_then(|voice_state| voice_state.channel_id)
        {
            None => return Ok(Err(Error::UserNotInVoiceChannel)),
            Some(channel_id) => channel_id,
        };

        let player = {
            let player = self
                .players
                .lock()
                .await
                .get(guild.id.as_ref())
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
                            return Ok(Err(Error::NotPlaying));
                        }
                        Ok(player_voice_channel_id) => {
                            if player_voice_channel_id != voice_channel_id.into() {
                                return Ok(Err(Error::UserInDifferentVoiceChannel));
                            }
                        }
                    }
                    player
                }
                _ => {
                    if command.data.name != "hrat" {
                        return Ok(Err(Error::NotPlaying));
                    } else {
                        match self
                            .create_player(guild.id, voice_channel_id, command.channel_id, context)
                            .await
                        {
                            Err(player::CreationError::ChannelJoin(error)) => {
                                let error = Error::CouldNotJoin(error);
                                error!("{error}");
                                return Ok(Err(error));
                            }
                            Err(error) => Err(error)?,
                            Ok(new_player) => new_player,
                        }
                    }
                }
            }
        };

        player.lock().await.set_text_channel_id(command.channel_id);

        let command_data_option = command.data.options.first();
        match command.data.name.as_str() {
            "hrat" => {
                let query = command_data_option
                    .and_then(|command_data_option| match &command_data_option.value {
                        CommandDataOptionValue::String(value) => Some(value),
                        _ => None,
                    })
                    .ok_or(ExecutionError::InvalidOption)?;
                Ok(Ok(Executed::Play(
                    self.play(&player, query)
                        .await
                        .map_err(ExecutionError::Play)?,
                )))
            }
            "fronta" => {
                let command_data_option =
                    command_data_option.ok_or(ExecutionError::InvalidOption)?;
                let subcommand_data_option_value = match &command_data_option.value {
                    CommandDataOptionValue::SubCommand(subcommand_data_options) => {
                        subcommand_data_options
                            .first()
                            .map(|subcommand_data_option| &subcommand_data_option.value)
                    }
                    _ => Err(ExecutionError::InvalidOption)?,
                };
                match command_data_option.name.as_str() {
                    "zobrazit" => Ok(Ok(Executed::QueueView(player.lock().await.queue().clone()))),
                    "posunout" => {
                        let index = subcommand_data_option_value
                            .and_then(|subcommand_data_option_value| {
                                match subcommand_data_option_value {
                                    CommandDataOptionValue::Integer(value) => value
                                        .checked_sub(1)
                                        .and_then(|value| usize::try_from(value).ok()),
                                    _ => None,
                                }
                            })
                            .ok_or(ExecutionError::InvalidOption)?;
                        Ok(player
                            .lock()
                            .await
                            .queue_move(index)
                            .await
                            .map(|_| Executed::QueueMove(index))
                            .map_err(Error::QueueMove))
                    }
                    "opakovat" => {
                        let repeat = subcommand_data_option_value
                            .and_then(|subcommand_data_option_value| {
                                match subcommand_data_option_value {
                                    CommandDataOptionValue::Boolean(value) => Some(*value),
                                    _ => None,
                                }
                            })
                            .ok_or(ExecutionError::InvalidOption)?;
                        player.lock().await.queue_repeat(repeat).await;
                        Ok(Ok(Executed::QueueRepeat(repeat)))
                    }
                    "nahodne" => {
                        player.lock().await.queue_shuffle().await;
                        Ok(Ok(Executed::QueueShuffle))
                    }
                    _ => panic!(),
                }
            }
            "dalsi" => Ok(player
                .lock()
                .await
                .next()
                .await
                .map(|_| Executed::Next)
                .map_err(Error::Next)),
            "predchozi" => Ok(player
                .lock()
                .await
                .previous()
                .await
                .map(|_| Executed::Previous)
                .map_err(Error::Previous)),
            "pauza" => Ok(Ok(player
                .lock()
                .await
                .pause()
                .await
                .map(|_| Executed::Pause)
                .map_err(ExecutionError::Pause)?)),
            "pokracovat" => Ok(Ok(player
                .lock()
                .await
                .resume()
                .await
                .map(|_| Executed::Resume)
                .map_err(ExecutionError::Resume)?)),
            "opakovat" => {
                let repeat = command
                    .data
                    .options
                    .first()
                    .and_then(|command_data_option| match command_data_option.value {
                        CommandDataOptionValue::Boolean(value) => Some(value),
                        _ => None,
                    })
                    .ok_or(ExecutionError::InvalidOption)?;
                player.lock().await.repeat(repeat).await;
                Ok(Ok(Executed::Repeat(repeat)))
            }
            "stop" => {
                player.lock().await.stop().await;
                Ok(Ok(Executed::Stop))
            }
            _ => Err(ExecutionError::InvalidOption),
        }
    }

    async fn create_player(
        &self,
        guild_id: GuildId,
        voice_channel_id: ChannelId,
        text_channel_id: ChannelId,
        context: Context,
    ) -> Result<Arc<Mutex<Player>>, player::CreationError> {
        let activity_manager = self.activity_manager.clone();
        let new_player = Player::new(
            self.http_client.clone(),
            guild_id,
            voice_channel_id,
            text_channel_id,
            context,
            move |track, text_channel_id, context| {
                Box::pin(Self::on_track_started_playing(
                    activity_manager.clone(),
                    track,
                    text_channel_id,
                    context,
                ))
            },
        )
        .await?;
        self.players
            .lock()
            .await
            .insert(guild_id, new_player.clone());

        Ok(new_player)
    }

    async fn play<'a>(
        &'a self,
        player: &Arc<Mutex<Player>>,
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

    async fn on_track_started_playing(
        activity_manager: Arc<RwLock<activity::Manager>>,
        track: Track,
        text_channel_id: ChannelId,
        context: Context,
    ) {
        _ = activity_manager
            .read()
            .await
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
