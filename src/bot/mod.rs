use crate::command::register_global_commands;
use crate::{activity, command};
use log::info;
use serenity::all::{Context, EditInteractionResponse, EventHandler, Interaction, Ready};
use serenity::async_trait;
use std::sync::Arc;
use tokio::sync::RwLock;
use unwrap_or_log::LogError;

pub(crate) struct Bot {
    command_executor: command::Executor,
    activity_manager: Arc<RwLock<activity::Manager>>,
}

impl Bot {
    pub(crate) async fn new(
        spotify_api_credentials: rspotify::Credentials,
    ) -> Result<Self, command::ExecutorCreationError> {
        let activity_manager = Arc::new(RwLock::new(activity::Manager::new()));
        Ok(Self {
            command_executor: command::Executor::new(
                spotify_api_credentials,
                activity_manager.clone(),
            )
            .await?,
            activity_manager,
        })
    }
}

#[async_trait]
impl EventHandler for Bot {
    async fn ready(&self, context: Context, _: Ready) {
        register_global_commands(&context)
            .await
            .log_error()
            .unwrap();

        info!("The bot is ready.");

        self.activity_manager.write().await.provide_context(context);
        self.activity_manager
            .read()
            .await
            .update_idle_activity()
            .await;
    }

    async fn interaction_create(&self, context: Context, interaction: Interaction) {
        let command_interaction = match interaction {
            Interaction::Command(command_interaction) => command_interaction,
            _ => return,
        };

        let cache_http = context.http.clone();

        _ = command_interaction.defer(&context).await.log_error();
        let embed = match self
            .command_executor
            .execute(context, &command_interaction)
            .await
            .log_error()
        {
            Err(error) => error.into(),
            Ok(Err(error)) => error.into(),
            Ok(Ok(executed_command)) => executed_command.into(),
        };

        _ = command_interaction
            .edit_response(cache_http, EditInteractionResponse::new().embed(embed))
            .await
            .log_error();
    }
}
