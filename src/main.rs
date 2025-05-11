use dotenv::dotenv;
use std::env;
use tranzistorak::run;

#[tokio::main]
async fn main() {
    dotenv().unwrap();

    let token = &env::var("DISCORD_TOKEN").unwrap();
    run(token).await;
}
