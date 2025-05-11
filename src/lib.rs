use crate::bot::Bot;
use crate::env::{DISCORD_API_TOKEN, SPOTIFY_API_CLIENT_ID, SPOTIFY_API_CLIENT_SECRET};
use crate::log::initialize_logger;
use serenity::Client;
use serenity::all::GatewayIntents;
use songbird::SerenityInit;
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

    let spotify_credentials =
        rspotify::Credentials::new(SPOTIFY_API_CLIENT_ID, SPOTIFY_API_CLIENT_SECRET);

    let bot = Bot::new(spotify_credentials).await.log_error().unwrap();

    Client::builder(DISCORD_API_TOKEN, DISCORD_INTENTS)
        .event_handler(bot)
        .register_songbird()
        .await
        .log_error()
        .unwrap()
        .start()
        .await
        .log_error()
        .unwrap();
}
