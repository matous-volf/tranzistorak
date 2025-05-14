use std::sync::Arc;
use std::time::Duration;

use crate::model::Track;
use serenity::client::Context;
use serenity::gateway::ActivityData;
use tokio::time::sleep;

const IDLE_ACTIVITIES_INTERVAL_SECONDS: u64 = 5 * 60;

pub(crate) struct Manager {
    context: Context,
}

impl Manager {
    pub(crate) fn new(context: Context) -> Arc<Self> {
        let new = Arc::new(Self { context });

        {
            let new = new.clone();
            tokio::spawn(async move {
                new.update_idle_activity().await;
            });
        }

        new
    }

    pub(crate) async fn set_current_playing_track(&self, track: Track) -> serenity::Result<()> {
        self.context.set_activity(Some(ActivityData::streaming(
            track.title,
            track.youtube_url,
        )?));

        Ok(())
    }

    pub(crate) async fn update_idle_activity(&self) {
        let idle_activities = [
            ActivityData::watching(format!("verze {}", crate::VERSION)),
            ActivityData::listening("/hrat"),
            ActivityData::playing("YouTube a Spotify"),
            ActivityData::playing("videa i playlisty"),
            ActivityData::watching("svobodný a otevřený software!"),
            ActivityData::watching("github.com/matous-volf/tranzistorak"),
        ];

        for activity in idle_activities.iter().cycle() {
            self.context.set_activity(Some(activity.clone()));
            sleep(Duration::from_secs(IDLE_ACTIVITIES_INTERVAL_SECONDS)).await;
        }
    }
}
