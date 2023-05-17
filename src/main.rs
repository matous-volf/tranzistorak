use tranzistorak::run;

#[tokio::main]
async fn main() {
    let token = "ODg3MzczMzkzNDEyOTExMTU0.GgT4U_.QqM2Xnp6MFm0geWKAlXUk89gxJ60GKJnjAMHlU";
    run(token).await;
}
