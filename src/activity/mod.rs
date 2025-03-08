use std::time::Duration;

use serenity::client::Context;
use serenity::gateway::ActivityData;
use tokio::time::sleep;

use crate::player::PlayerTrack;

const IDLE_ACTIVITIES_INTERVAL_SECONDS: u64 = 5 * 60;

pub struct ActivityHandler {}

impl ActivityHandler {
    pub async fn set_current_playing_track(track: PlayerTrack, context: Context) {
        context.set_activity(Some(ActivityData::streaming(track.title(), track.url()).unwrap()));
    }

    pub async fn update_activity(context: Context) {
        let mut idle_index = 0;

        let idle_activities = vec![
            ActivityData::watching(format!("verze {}", crate::VERSION)),
            ActivityData::listening("/hrat"),
            ActivityData::playing("YouTube a Spotify"),
            ActivityData::playing("videa i playlisty"),
            ActivityData::playing("svobodný a otevřený software!"),
            ActivityData::playing("github.com/matous-volf/tranzistorak"),
        ];

        loop {
            context.set_activity(Some(idle_activities[idle_index].clone()));
            idle_index += 1;
            if idle_index >= idle_activities.len() {
                idle_index = 0;
            }

            sleep(Duration::from_secs(IDLE_ACTIVITIES_INTERVAL_SECONDS)).await;
        }
    }
}
