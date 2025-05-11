use std::sync::Arc;

use serenity::all::{Command, CommandOptionType, Interaction};
use serenity::async_trait;
use serenity::builder::{CreateCommand, CreateCommandOption, EditInteractionResponse};
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use songbird::SerenityInit;

use activity::ActivityHandler;
use commands::CommandHandler;

mod activity;
mod commands;
mod embeds;
mod player;
mod youtube;

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

        Command::set_global_commands(&context.http, vec![
            CreateCommand::new("hrat")
                .description("Zařadí do fronty položku z odkazu nebo hledání.")
                .set_options(vec![
                    CreateCommandOption::new(
                        CommandOptionType::String, "hledani", "odkaz nebo text k vyhledání",
                    ).required(true)
                ])
                .dm_permission(false),
            CreateCommand::new("dalsi")
                .description("Přeskočí přehrávání na další pozici ve frontě.")
                .dm_permission(false),
            CreateCommand::new("predchozi")
                .description("Vrátí přehrávání na předchozí pozici ve frontě.")
                .dm_permission(false),
            CreateCommand::new("fronta")
                .description("Slouží k akcím s frontou přehrávání.")
                .set_options(vec![
                    CreateCommandOption::new(
                        CommandOptionType::SubCommand,
                        "zobrazit",
                        "Vypíše všechny položky ve frontě.",
                    ),
                    CreateCommandOption::new(
                        CommandOptionType::SubCommand,
                        "posunout",
                        "Posune přehrávání na zadanou pozici ve frontě.",
                    ).set_sub_options(vec![
                        CreateCommandOption::new(
                            CommandOptionType::Integer,
                            "pozice",
                            "pozice ve frontě k posunutí",
                        ).required(true).min_int_value(1),
                    ]),
                    CreateCommandOption::new(
                        CommandOptionType::SubCommand,
                        "opakovat",
                        "Zapne nebo vypne opakování fronty.",
                    ).set_sub_options(vec![
                        CreateCommandOption::new(
                            CommandOptionType::Boolean,
                            "zapnout",
                            "zda zapnout opakování",
                        ).required(true),
                    ]),
                    CreateCommandOption::new(
                        CommandOptionType::SubCommand,
                        "nahodne",
                        "Náhodně zamíchá frontu a začne přehrávat od první položky.",
                    ),
                ])
                .dm_permission(false),
            CreateCommand::new("pauza")
                .description("Pozastaví přehrávání.")
                .dm_permission(false),
            CreateCommand::new("pokracovat")
                .description("Znovu spustí pozastavené přehrávání.")
                .dm_permission(false),
            CreateCommand::new("opakovat")
                .description("Zapne nebo vypne opakování aktuální položky.")
                .set_options(vec![
                    CreateCommandOption::new(
                        CommandOptionType::Boolean,
                        "zapnout",
                        "zda zapnout opakování",
                    ).required(true)
                ])
                .dm_permission(false),
            CreateCommand::new("stop")
                .description("Zastaví přehrávání, odstraní všechny položky ve frontě a opustí hlasový kanál.")
                .dm_permission(false),
        ],
        ).await.unwrap();

        ActivityHandler::update_activity(context).await;
    }

    async fn interaction_create(&self, context: Context, interaction: Interaction) {
        if let Interaction::Command(command) = interaction {
            _ = command.defer(&context).await;

            let mut command_handler = self.command_handler.lock().await;
            let response = command_handler.handle(&context, &command);

            let embed = response.await;

            _ = command
                .edit_response(&context.http, EditInteractionResponse::new().embed(embed))
                .await;
        }
    }
}

pub async fn run(token: &str) {
    tracing_subscriber::fmt::init();

    let mut client = Client::builder(token, DISCORD_INTENTS)
        .event_handler(Handler::new().await)
        .register_songbird()
        .await
        .unwrap();

    client.start().await.unwrap();
}
