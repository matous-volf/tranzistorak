use crate::command::{
    Command, Error, FromInteractionError, UserCausedError, register_global_commands,
};
use crate::{activity, command, player};
use amplify_derive::Display;
use log::{error, info};
use serenity::all::{
    Context, CreateEmbed, CreateMessage, EditInteractionResponse, EventHandler, GuildId,
    Interaction, Ready,
};
use serenity::async_trait;
use songbird::events::context_data::VoiceTick;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use unwrap_or_log::LogError;

#[derive(Error, Display, Debug)]
#[display(Debug)]
#[allow(dead_code)]
pub(crate) enum CreationError {
    CommandExecutor(command::ExecutorCreationError),
    VoiceCommandHandler(command::voice::TranscriptorCreationError),
}

pub(crate) struct Bot {
    context: Context,
    command_executor: Arc<command::Executor<Arc<Self>>>,
    /* It is not needed to store these two in this struct, but this way they are noted as a part of
    the state. */
    #[allow(dead_code)]
    voice_command_transcriptor: Arc<command::voice::Transcriptor<Arc<Self>>>,
    #[allow(dead_code)]
    activity_manager: Arc<activity::Manager>,
}

impl Bot {
    pub(crate) async fn new(context: Context) -> Result<Arc<Self>, CreationError> {
        let activity_manager = activity::Manager::new(context.clone());

        let command_executor = Arc::new(
            command::Executor::new(None, activity_manager.clone())
                .await
                .map_err(CreationError::CommandExecutor)?,
        );
        let voice_command_transcriptor = command::voice::Transcriptor::new(None)
            .await
            .map_err(CreationError::VoiceCommandHandler)?;

        let new = Arc::new(Self {
            context,
            command_executor,
            voice_command_transcriptor,
            activity_manager,
        });

        new.command_executor
            .set_voice_tick_callback(Some(new.clone()))
            .await;
        new.voice_command_transcriptor
            .set_voice_transcribed_callback(Some(new.clone()))
            .await;

        Ok(new)
    }

    async fn on_interaction_create(&self, context: Context, interaction: Interaction) {
        let command_interaction = match interaction {
            Interaction::Command(command_interaction) => command_interaction,
            _ => return,
        };

        let cache_http = context.http.clone();

        _ = command_interaction.defer(&context).await.log_error();
        let embed = match Command::try_from_interaction(&command_interaction, &context).await {
            Err(error) => match error {
                FromInteractionError::UserCaused(error) => error.into(),
                FromInteractionError::Internal(error) => {
                    error!("{error}");
                    error.into()
                }
            },
            Ok(command) => self.execute_command(context, &command).await,
        };

        _ = command_interaction
            .edit_response(cache_http, EditInteractionResponse::new().embed(embed))
            .await
            .log_error();
    }

    async fn execute_command(&self, context: Context, command: &Command) -> CreateEmbed {
        match self.command_executor.execute(context, command).await {
            Err(error) => match error {
                Error::UserCaused(error) => {
                    if let UserCausedError::CouldNotJoin(error) = &error {
                        error!("{error}");
                    }
                    error.into()
                }
                Error::Internal(error) => {
                    error!("{error}");
                    error.into()
                }
            },
            Ok(executed_command) => executed_command.into(),
        }
    }
}

#[async_trait]
impl player::VoiceTickCallback for Arc<Bot> {
    async fn on_voice_tick(&self, guild_id: GuildId, voice_tick: VoiceTick) {
        self.voice_command_transcriptor
            .process_voice_tick(guild_id, &voice_tick)
            .await;
    }
}

#[async_trait]
impl command::voice::VoiceTranscribedCallback for Arc<Bot> {
    async fn on_voice_transcribed(&self, guild_id: GuildId, text: String) {
        let text_channel_id = match self
            .command_executor
            .player_text_channel_id(&guild_id)
            .await
        {
            None => return,
            Some(text_channel_id) => text_channel_id,
        };

        let command = match Command::try_from_text(text, guild_id) {
            Err(_) => return,
            Ok(command) => command,
        };

        let embed = self.execute_command(self.context.clone(), &command).await;
        let _ = text_channel_id
            .send_message(self.context.http.clone(), CreateMessage::new().embed(embed))
            .await
            .log_error();
    }
}

pub(crate) struct Handler {
    bot: RwLock<Option<Arc<Bot>>>,
}

impl Handler {
    pub(crate) fn new() -> Self {
        Self {
            bot: RwLock::new(None),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, context: Context, _: Ready) {
        register_global_commands(&context)
            .await
            .log_error()
            .unwrap();

        info!("The bot is ready.");

        *self.bot.write().await = Some(Bot::new(context).await.log_error().unwrap());
    }

    async fn interaction_create(&self, context: Context, interaction: Interaction) {
        let bot = self.bot.read().await;
        let bot = match bot.as_ref() {
            None => return,
            Some(bot) => bot,
        };
        bot.on_interaction_create(context, interaction).await;
    }
}
