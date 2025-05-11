use load_dotenv::load_dotenv;

load_dotenv!();

pub(crate) const DISCORD_API_TOKEN: &str = env!("DISCORD_API_TOKEN");
pub(crate) const SPOTIFY_API_CLIENT_ID: &str = env!("SPOTIFY_API_CLIENT_ID");
pub(crate) const SPOTIFY_API_CLIENT_SECRET: &str = env!("SPOTIFY_API_CLIENT_SECRET");
