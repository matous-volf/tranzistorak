use std::time::Duration;
use serenity::client::Context;
use serenity::model::gateway::Activity;
use tokio::time::sleep;
use crate::player::PlayerTrack;

const IDLE_ACTIVITIES_INTERVAL_SECONDS: u64 = 5 * 60;

pub struct ActivityHandler {}

impl ActivityHandler {
    pub async fn set_current_playing_track(track: PlayerTrack, context: Context) {
        context.set_activity(Activity::streaming(track.title(), track.url())).await;
    }

    pub async fn update_activity(context: Context) {
        let mut idle_index = 0;

        let idle_activities = vec![
            Activity::watching(format!("verze {}", crate::VERSION)),
            Activity::listening("/hrat"),
            Activity::playing("YouTube a Spotify"),
            Activity::playing("videa i playlisty"),
            Activity::playing("nově opensource!"),
            Activity::playing("github.com/matous-volf/tranzistorak"),
        ];

        loop {
            context.set_activity(idle_activities[idle_index].clone()).await;
            idle_index += 1;
            if idle_index >= idle_activities.len() {
                idle_index = 0;
            }

            sleep(Duration::from_secs(IDLE_ACTIVITIES_INTERVAL_SECONDS)).await;
        }
    }
}
