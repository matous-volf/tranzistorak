mod execution;
mod registration;
pub(crate) mod voice;

use amplify_derive::Display;
use deunicode::deunicode;
pub(crate) use execution::*;
pub(crate) use registration::*;
use serenity::all::{ChannelId, CommandDataOptionValue, CommandInteraction, Context, GuildId};
use std::str::FromStr;
use thiserror::Error;

enum Action {
    Play {
        text_channel_id: ChannelId,
        voice_channel_id: ChannelId,
        query: String,
    },
    VoicePlay {
        query: String,
    },
    QueueView,
    QueueMove {
        index: usize,
    },
    QueueRepeat(bool),
    QueueShuffle,
    Next,
    Previous,
    Pause,
    Resume,
    Repeat(bool),
    Stop,
}

impl FromStr for Action {
    type Err = ();

    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let mut words = text.split_whitespace();

        enum Stage {
            None,
            Base,
            Queue,
            QueueMove,
            QueueRepeat,
            Repeat,
        }
        let mut current_stage = Stage::None;

        while let Some(word) = words.next() {
            if word.is_empty() {
                continue;
            }

            let word_normalized = {
                let mut word = deunicode(word);
                word.make_ascii_lowercase();
                word
            };

            match current_stage {
                Stage::None => {
                    if word_normalized.replace("z", "s").contains("trans")
                        || word_normalized.replace("z", "s").contains("istor")
                    {
                        current_stage = Stage::Base;
                    }
                }
                Stage::Base => {
                    if word_normalized.replace("d", "t").contains("rat")
                        || word_normalized.contains("hra")
                    {
                        let remaining_words = words.collect::<Vec<_>>();
                        if remaining_words.is_empty() {
                            Err(())?;
                        }
                        return Ok(Self::VoicePlay {
                            query: remaining_words.join(" "),
                        });
                    } else if word_normalized.contains("front") {
                        current_stage = Stage::Queue;
                    } else if word_normalized.replace("t", "d").contains("dal") {
                        return Ok(Self::Next);
                    } else if word_normalized.replace("t", "d").contains("pred")
                        || word_normalized.replace("s", "z").contains("hoz")
                    {
                        return Ok(Self::Previous);
                    } else if word_normalized.contains("pau") {
                        return Ok(Self::Pause);
                    } else if word_normalized.replace("g", "k").contains("krac") {
                        return Ok(Self::Resume);
                    } else if word_normalized.contains("pako") {
                        current_stage = Stage::Repeat;
                    } else if word_normalized.replace("d", "t").contains("top") {
                        return Ok(Self::Stop);
                    }
                }
                Stage::Queue => {
                    if word_normalized.replace("s", "z").contains("obraz") {
                        return Ok(Self::QueueView);
                    } else if word_normalized.contains("suno") {
                        current_stage = Stage::QueueMove;
                    } else if word_normalized.contains("pako") {
                        current_stage = Stage::QueueRepeat;
                    } else if word_normalized.replace("t", "d").contains("hod") {
                        return Ok(Self::QueueShuffle);
                    }
                }
                Stage::QueueMove => {
                    if let Some(index) = word_normalized
                        .chars()
                        .filter(|char| char.is_numeric())
                        .collect::<String>()
                        .parse::<usize>()
                        .ok()
                        .and_then(|index| index.checked_sub(1))
                    {
                        return Ok(Self::QueueMove { index });
                    }
                }
                Stage::QueueRepeat => {
                    if word_normalized.contains("ano") {
                        return Ok(Self::QueueRepeat(true));
                    } else if word_normalized.contains("ne") {
                        return Ok(Self::QueueRepeat(false));
                    }
                }
                Stage::Repeat => {
                    if word_normalized.contains("ano") {
                        return Ok(Self::Repeat(true));
                    } else if word_normalized.contains("ne") {
                        return Ok(Self::Repeat(false));
                    }
                }
            }
        }

        Err(())
    }
}

#[derive(Error, Display, Debug)]
#[display(Debug)]
#[allow(dead_code)]
pub(crate) enum FromInteractionError {
    UserCaused(FromInteractionUserCausedError),
    Internal(FromInteractionInternalError),
}

impl From<FromInteractionUserCausedError> for FromInteractionError {
    fn from(from_interaction_user_caused_error: FromInteractionUserCausedError) -> Self {
        Self::UserCaused(from_interaction_user_caused_error)
    }
}

impl From<FromInteractionInternalError> for FromInteractionError {
    fn from(from_interaction_internal_error: FromInteractionInternalError) -> Self {
        Self::Internal(from_interaction_internal_error)
    }
}

#[derive(Error, Display, Debug)]
#[display(Debug)]
pub(crate) enum FromInteractionUserCausedError {
    NotInGuild,
    UserNotInVoiceChannel,
}

#[derive(Error, Display, Debug)]
#[display(Debug)]
pub(crate) enum FromInteractionInternalError {
    GuildNotFoundById,
    InvalidOption,
}

pub(crate) struct Command {
    guild_id: GuildId,
    /// Not set for voice commands.
    voice_channel_id: Option<ChannelId>,
    text_channel_id: Option<ChannelId>,
    action: Action,
}

impl Command {
    pub(crate) async fn try_from_interaction(
        command_interaction: &CommandInteraction,
        context: &Context,
    ) -> Result<Command, FromInteractionError> {
        let guild = match command_interaction.guild_id {
            None => Err(FromInteractionUserCausedError::NotInGuild)?,
            Some(guild_id) => guild_id
                .to_guild_cached(context.cache.as_ref())
                .ok_or(FromInteractionInternalError::GuildNotFoundById)?
                .clone(),
        };

        let voice_channel_id = match guild
            .voice_states
            .get(&command_interaction.user.id)
            .and_then(|voice_state| voice_state.channel_id)
        {
            None => Err(FromInteractionUserCausedError::UserNotInVoiceChannel)?,
            Some(channel_id) => channel_id,
        };

        let command_data_option = command_interaction.data.options.first();
        let action = match command_interaction.data.name.as_str() {
            "hrat" => {
                let query = command_data_option
                    .and_then(|command_data_option| match &command_data_option.value {
                        CommandDataOptionValue::String(value) => Some(value),
                        _ => None,
                    })
                    .ok_or(FromInteractionInternalError::InvalidOption)?;
                Action::Play {
                    text_channel_id: command_interaction.channel_id,
                    voice_channel_id,
                    query: query.clone(),
                }
            }
            "fronta" => {
                let command_data_option =
                    command_data_option.ok_or(FromInteractionInternalError::InvalidOption)?;
                let subcommand_data_option_value = match &command_data_option.value {
                    CommandDataOptionValue::SubCommand(subcommand_data_options) => {
                        subcommand_data_options
                            .first()
                            .map(|subcommand_data_option| &subcommand_data_option.value)
                    }
                    _ => Err(FromInteractionInternalError::InvalidOption)?,
                };
                match command_data_option.name.as_str() {
                    "zobrazit" => Action::QueueView,
                    "posunout" => {
                        let index = subcommand_data_option_value
                            .and_then(|subcommand_data_option_value| {
                                match subcommand_data_option_value {
                                    CommandDataOptionValue::Integer(value) => value
                                        .checked_sub(1)
                                        .and_then(|value| usize::try_from(value).ok()),
                                    _ => None,
                                }
                            })
                            .ok_or(FromInteractionInternalError::InvalidOption)?;
                        Action::QueueMove { index }
                    }
                    "opakovat" => {
                        let repeat = subcommand_data_option_value
                            .and_then(|subcommand_data_option_value| {
                                match subcommand_data_option_value {
                                    CommandDataOptionValue::Boolean(value) => Some(*value),
                                    _ => None,
                                }
                            })
                            .ok_or(FromInteractionInternalError::InvalidOption)?;
                        Action::QueueRepeat(repeat)
                    }
                    "nahodne" => Action::QueueShuffle,
                    _ => Err(FromInteractionInternalError::InvalidOption)?,
                }
            }
            "dalsi" => Action::Next,
            "predchozi" => Action::Previous,
            "pauza" => Action::Pause,
            "pokracovat" => Action::Resume,
            "opakovat" => {
                let repeat = command_data_option
                    .and_then(|command_data_option| match command_data_option.value {
                        CommandDataOptionValue::Boolean(value) => Some(value),
                        _ => None,
                    })
                    .ok_or(FromInteractionInternalError::InvalidOption)?;
                Action::Repeat(repeat)
            }
            "stop" => Action::Stop,
            _ => Err(FromInteractionInternalError::InvalidOption)?,
        };

        Ok(Self {
            guild_id: guild.id,
            voice_channel_id: Some(voice_channel_id),
            text_channel_id: Some(command_interaction.channel_id),
            action,
        })
    }

    pub(crate) fn try_from_text(text: impl AsRef<str>, guild_id: GuildId) -> Result<Self, ()> {
        Ok(Self {
            guild_id,
            voice_channel_id: None,
            text_channel_id: None,
            action: Action::from_str(text.as_ref())?,
        })
    }
}
