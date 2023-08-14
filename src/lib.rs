mod player;
mod commands;
mod embeds;
mod activity;
mod youtube;

use std::sync::Arc;
use serenity::async_trait;
use serenity::model::application::command::{Command, CommandOptionType};
use serenity::model::application::interaction::{Interaction, InteractionResponseType};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use songbird::{SerenityInit};
use activity::ActivityHandler;

use commands::CommandHandler;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DISCORD_INTENTS: GatewayIntents = GatewayIntents::non_privileged();

struct Handler {
    command_handler: Arc<Mutex<CommandHandler>>,
}

impl Handler {
    async fn new() -> Handler {
        Handler {
            command_handler: Arc::new(Mutex::new(CommandHandler::new().await)),
        }
    }
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, context: Context, _: Ready) {
        println!("Ready.");

        Command::set_global_application_commands(&context.http, |commands| {
            commands
                .create_application_command(|command| command
                    .name("hrat")
                    .description("Zařadí do fronty položku z odkazu nebo hledání.")
                    .create_option(|option| {
                        option
                            .name("hledani")
                            .description("odkaz nebo text k vyhledání")
                            .kind(CommandOptionType::String)
                            .required(true)
                    })
                    .dm_permission(false)
                )

                .create_application_command(|command| command
                    .name("dalsi")
                    .dm_permission(false)
                    .description("Přeskočí přehrávání na další pozici ve frontě."))

                .create_application_command(|command| command
                    .name("predchozi")
                    .dm_permission(false)
                    .description("Vrátí přehrávání na předchozí pozici ve frontě."))

                .create_application_command(|command| command
                    .name("fronta")
                    .description("Slouží k akcím s frontou přehrávání.")
                    .create_option(|option| {
                        option
                            .name("zobrazit")
                            .description("Vypíše všechny položky ve frontě.")
                            .kind(CommandOptionType::SubCommand)
                    })
                    .create_option(|option| option
                        .name("posunout")
                        .description("Posune přehrávání na zadanou pozici ve frontě.")
                        .kind(CommandOptionType::SubCommand)
                        .create_sub_option(|option| {
                            option
                                .name("pozice")
                                .description("pozice ve frontě k posunutí")
                                .kind(CommandOptionType::Integer)
                                .min_int_value(1)
                                .required(true)
                        }))
                    .create_option(|option| option
                        .name("opakovat")
                        .description("Zapne nebo vypne opakování fronty.")
                        .kind(CommandOptionType::SubCommand)
                        .create_sub_option(|option| option
                            .name("zapnout")
                            .description("zda zapnout opakování")
                            .kind(CommandOptionType::Boolean)
                            .required(true)))
                    .create_option(|option| option
                        .name("nahodne")
                        .description("Náhodně zamíchá frontu a začne přehrávat od první položky.")
                        .kind(CommandOptionType::SubCommand))
                    .dm_permission(false)
                )
                .create_application_command(|command| command
                    .name("pauza")
                    .dm_permission(false)
                    .description("Pozastaví přehrávání."))

                .create_application_command(|command| command
                    .name("pokracovat")
                    .description("Znovu spustí pozastavené přehrávání.")
                    .dm_permission(false))

                .create_application_command(|command| command
                    .name("opakovat")
                    .description("Zapne nebo vypne opakování aktuální položky.")
                    .create_option(|option| option
                        .name("zapnout")
                        .description("zda zapnout opakování")
                        .kind(CommandOptionType::Boolean)
                        .required(true))
                    .dm_permission(false))

                .create_application_command(|command| command
                    .name("stop")
                    .description("Zastaví přehrávání, odstraní všechny položky ve frontě a opustí hlasový kanál.")
                    .dm_permission(false))
        }).await.unwrap();

        ActivityHandler::update_activity(context).await;
    }

    async fn interaction_create(&self, context: Context, interaction: Interaction) {
        if let Interaction::ApplicationCommand(command) = interaction {
            _ = command
                .create_interaction_response(&context.http, |response| {
                    response
                        .kind(InteractionResponseType::DeferredChannelMessageWithSource)
                })
                .await;

            let mut command_handler = self.command_handler.lock().await;
            let response = command_handler.handle(&context, &command);

            let embed = response.await;

            _ = command.edit_original_interaction_response(&context.http, |response| response
                .set_embed(embed)).await;
        }
    }
}

pub async fn run(token: &str) {
    let mut client = Client::builder(token, DISCORD_INTENTS)
        .event_handler(Handler::new().await)
        .register_songbird()
        .await
        .unwrap();

    client.start().await.unwrap();
}
