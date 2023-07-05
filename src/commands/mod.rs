use std::cmp::{max, min};
use std::collections::{HashMap};
use std::sync::Arc;
use serenity::builder::CreateEmbed;
use serenity::client::Context;
use serenity::model::application::interaction::application_command::ApplicationCommandInteraction;
use serenity::model::id::{ChannelId, GuildId};
use serenity::model::prelude::interaction::application_command::CommandDataOptionValue;
use tokio::sync::Mutex;
use crate::activity::ActivityHandler;

use crate::player::{Player, PlayerTrack};
use crate::embeds;
use crate::embeds::{EmbedIcon};

const QUEUE_VIEW_MAX_TRACKS: usize = 15;

pub struct CommandHandler {
    players: HashMap<GuildId, Arc<Mutex<Player>>>,
}

impl CommandHandler {
    pub async fn new() -> CommandHandler {
        CommandHandler {
            players: HashMap::new(),
        }
    }

    pub async fn handle(&mut self, context: &Context, command: &ApplicationCommandInteraction) -> CreateEmbed {
        let guild = command.guild_id.unwrap().to_guild_cached(&context.cache).ok_or(()).unwrap();
        let text_channel_id = command.channel_id;

        let voice_channel_id = guild
            .voice_states.get(&command.user.id)
            .and_then(|voice_state| voice_state.channel_id);

        let voice_channel_id = match voice_channel_id {
            Some(channel) => channel,
            None => {
                return embeds::error("Ovládání", "Pro ovládání je nutné se připojit do hlasového kanálu.");
            }
        };

        let player = self.players.get(&guild.id);

        if player.is_none() || player.unwrap().lock().await.is_stopped().await {
            if command.data.name != "hrat" {
                return embeds::error("Ovládání", "Neprobíhá přehrávání.");
            } else {
                if let Err(_) = self.create_player(
                    guild.id,
                    voice_channel_id,
                    text_channel_id,
                    context,
                ).await {
                    return embeds::error("Chyba", "Nebylo možné připojit se do hlasového kanálu.");
                }
            }
        } else if let Some(player) = player {
            if player.lock().await.voice_channel_id().await.0 != voice_channel_id.0 {
                return embeds::error(
                    "Ovládání",
                    "Pro ovládání je nutné se připojit do hlasového kanálu, v němž se nachází bot.",
                );
            }
        }

        let player = self.players.get(&guild.id).unwrap();
        player.lock().await.set_text_channel_id(text_channel_id);

        match command.data.name.as_str() {
            "hrat" => {
                let query = match command.data.options[0].resolved.as_ref().unwrap() {
                    CommandDataOptionValue::String(string) => string,
                    _ => panic!(),
                };

                self.play(player, query).await
            }
            "fronta" => {
                match command.data.options[0].name.as_str() {
                    "zobrazit" => self.queue_view(player).await,
                    "posunout" => {
                        let index = match command.data.options[0].options[0].resolved.as_ref().unwrap() {
                            CommandDataOptionValue::Integer(integer) => (*integer - 1) as usize,
                            _ => panic!(),
                        };

                        self.queue_move(player, index).await
                    }
                    "opakovat" => {
                        let repeat = match command.data.options[0].options[0].resolved.as_ref().unwrap() {
                            CommandDataOptionValue::Boolean(boolean) => *boolean,
                            _ => panic!(),
                        };

                        self.queue_repeat(player, repeat).await
                    }
                    "nahodne" => self.queue_shuffle(player).await,
                    _ => panic!(),
                }
            }
            "dalsi" => self.next(player).await,
            "predchozi" => self.previous(player).await,
            "pauza" => self.pause(player).await,
            "pokracovat" => self.resume(player).await,
            "opakovat" => {
                let repeat = match command.data.options[0].resolved.as_ref().unwrap() {
                    CommandDataOptionValue::Boolean(boolean) => *boolean,
                    _ => panic!(),
                };

                self.repeat(player, repeat).await
            }
            "stop" => self.stop(player).await,
            _ => panic!()
        }
    }

    async fn create_player(&mut self, guild_id: GuildId, voice_channel_id: ChannelId, text_channel_id: ChannelId, context: &Context) -> Result<(), ()> {
        let new_player = Player::new(
            guild_id,
            voice_channel_id,
            text_channel_id,
            context).await;

        let new_player = match new_player {
            Ok(player) => player,
            Err(_) => return Err(()),
        };

        self.players.insert(guild_id, new_player).as_ref();

        Ok(())
    }

    async fn play(&self, player: &Arc<Mutex<Player>>, query: &str) -> CreateEmbed {
        let mut player = player.lock().await;
        let track = player.enqueue(query).await;

        let search_result = match track {
            Some(track) => track,
            None => return embeds::error("Nenalezeno", "Dle zadaného textu nebyl nalezen žádný výsledek.")
        };

        let mut embed = embeds::base("Přidáno do fronty", EmbedIcon::Queue, search_result.title());
        embed.url(search_result.url());
        embed.thumbnail(search_result.thumbnail_url());
        embed
    }

    pub async fn track_started_playing(player: &Player, track: &PlayerTrack, context: Context) {
        ActivityHandler::set_current_playing_track(track.clone(), context.clone()).await;

        let mut embed = embeds::base("Přehrávání", EmbedIcon::YouTube, track.title());
        embed.url(track.url());
        embed.thumbnail(track.thumbnail_url());
        _ = player.text_channel_id().send_message(context.http, |message| message.set_embed(embed)).await;
    }

    async fn next(&self, player: &Arc<Mutex<Player>>) -> CreateEmbed {
        match player.lock().await.next().await {
            Err(_) => embeds::error("Ovládání", "Ve frontě se nenachází žádné další položky."),
            Ok(()) => embeds::base("Ovládání", EmbedIcon::Next, "Přehrávání další položky.")
        }
    }

    async fn previous(&self, player: &Arc<Mutex<Player>>) -> CreateEmbed {
        match player.lock().await.previous().await {
            Err(_) => embeds::error("Ovládání", "Ve frontě se nenachází žádné předchozí položky."),
            Ok(()) => embeds::base("Ovládání", EmbedIcon::Previous, "Přehrávání předchozí položky.")
        }
    }

    async fn queue_view(&self, player: &Arc<Mutex<Player>>) -> CreateEmbed {
        let player = player.lock().await;
        let queue = player.queue().await;
        let current_playing_index = player.current_playing_index();

        let index = current_playing_index.unwrap_or(0);
        let start = max(0, index as i32 - QUEUE_VIEW_MAX_TRACKS as i32) as usize;
        let end = min(queue.len(), index + QUEUE_VIEW_MAX_TRACKS);

        let mut queue_text = String::new();

        if start > 0 {
            queue_text.push_str(format!("*předcházejících: {}*\n", start).as_str());
        }

        for (i, track) in queue.iter().skip(start).take(end - start).enumerate() {
            let mut track_text = format!("{}. [{}]({})\n", start + i + 1, track.title(), track.url());

            if let Some(index) = current_playing_index {
                if index == start + i {
                    track_text = format!("**{}**", track_text)
                }
            }

            queue_text.push_str(track_text.as_str());
        }

        if end != queue.len() {
            queue_text.push_str(format!("*následjících: {}*\n", queue.len() - end).as_str());
        }

        let mut embed = embeds::base("Fronta", EmbedIcon::Queue, "Položky ve frontě:");
        embed.description(queue_text);

        embed
    }

    async fn queue_move(&self, player: &Arc<Mutex<Player>>, index: usize) -> CreateEmbed {
        match player.lock().await.queue_move(index).await {
            Err(_) => {
                embeds::error("Fronta", format!("Fronta {}. pozici neobsahuje.", index + 1).as_str())
            }
            Ok(()) => {
                embeds::base("Fronta", EmbedIcon::Queue, format!("Přehrávání posunuto na {}. pozici ve frontě.", index + 1).as_str())
            }
        }
    }

    async fn queue_repeat(&self, player: &Arc<Mutex<Player>>, repeat: bool) -> CreateEmbed {
        player.lock().await.queue_repeat(repeat).await;
        embeds::base("Ovládání", EmbedIcon::Repeat,
                     format!("Opakované přehrávání fronty je {}.", if repeat { "zapnuto" } else { "vypnuto" }).as_str())
    }

    async fn queue_shuffle(&self, player: &Arc<Mutex<Player>>) -> CreateEmbed {
        player.lock().await.queue_shuffle().await;
        embeds::base("Ovládání", EmbedIcon::Repeat,
                     "Fronta byla náhodně zamíchána.")
    }

    async fn pause(&self, player: &Arc<Mutex<Player>>) -> CreateEmbed {
        player.lock().await.pause().await;
        embeds::base("Ovládání", EmbedIcon::Pause, "Přehrávání pozastaveno.")
    }

    async fn resume(&self, player: &Arc<Mutex<Player>>) -> CreateEmbed {
        player.lock().await.resume().await;
        embeds::base("Ovládání", EmbedIcon::Resume, "Přehrávání pokračuje.")
    }

    async fn repeat(&self, player: &Arc<Mutex<Player>>, repeat: bool) -> CreateEmbed {
        player.lock().await.repeat(repeat).await;
        embeds::base("Ovládání", EmbedIcon::Repeat,
                     format!("Opakované přehrávání aktuální položky je {}.", if repeat { "zapnuto" } else { "vypnuto" }).as_str())
    }

    async fn stop(&self, player: &Arc<Mutex<Player>>) -> CreateEmbed {
        player.lock().await.stop().await;
        embeds::base("Ovládání", EmbedIcon::Stop, "Přehrávání zastaveno.")
    }
}
