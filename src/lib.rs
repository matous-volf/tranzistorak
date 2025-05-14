use crate::env::DISCORD_API_TOKEN;
use crate::log::initialize_logger;
use serenity::Client;
use serenity::all::GatewayIntents;
use songbird::driver::DecodeMode;
use songbird::{Config, SerenityInit};
use unwrap_or_log::LogError;

mod activity;
mod bot;
mod command;
mod embed;
mod env;
mod log;
mod model;
mod player;
mod query;
mod utils;
mod youtube;

const VERSION: &str = env!("CARGO_PKG_VERSION");
const DISCORD_INTENTS: GatewayIntents = GatewayIntents::non_privileged();

pub async fn run() {
    initialize_logger().unwrap_or_else(|error| panic!("on initializing the logger: {}", error));

    let handler = bot::Handler::new();

    Client::builder(DISCORD_API_TOKEN, DISCORD_INTENTS)
        .event_handler(handler)
        .register_songbird_from_config(
            Config::default()
                .decode_mode(DecodeMode::Decode)
                .decode_channels(songbird::driver::Channels::Mono)
                .decode_sample_rate(command::voice::SAMPLE_RATE),
        )
        .await
        .log_error()
        .unwrap()
        .start()
        .await
        .log_error()
        .unwrap();
}
