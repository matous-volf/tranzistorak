use crate::command::{FromInteractionInternalError, FromInteractionUserCausedError};
use crate::{command, player};
use serenity::builder::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter};
use serenity::model::Color;

const ICONS_BASE_URL: &str =
    "https://raw.githubusercontent.com/matous-volf/tranzistorak/main/icons/";
const QUEUE_VIEW_MAX_TRACKS: usize = 15;

pub(crate) fn base(
    author_text: impl Into<String>,
    author_icon: EmbedIcon,
    title: impl Into<String>,
) -> CreateEmbed {
    CreateEmbed::new()
        .footer(
            CreateEmbedFooter::new(format!("Tranzistorák v{}", crate::VERSION))
                .icon_url(EmbedIcon::Bot.url()),
        )
        .author(CreateEmbedAuthor::new(author_text).icon_url(author_icon.url()))
        .title(title)
}

pub(crate) fn error(author_text: impl Into<String>, title: impl Into<String>) -> CreateEmbed {
    base(author_text, EmbedIcon::Error, title).color(Color::RED)
}

pub(crate) fn command_generic_error() -> CreateEmbed {
    error("Chyba", "Při vykonávání příkazu nastala chyba.")
}

pub(crate) enum EmbedIcon {
    Bot,
    YouTube,
    Error,
    Queue,
    Next,
    Previous,
    Pause,
    Resume,
    Repeat,
    Stop,
}

impl EmbedIcon {
    fn url(&self) -> String {
        let icon = match self {
            EmbedIcon::Bot => "bot",
            EmbedIcon::YouTube => "youtube",
            EmbedIcon::Error => "error",
            EmbedIcon::Queue => "queue",
            EmbedIcon::Next => "next",
            EmbedIcon::Previous => "previous",
            EmbedIcon::Pause => "pause",
            EmbedIcon::Resume => "resume",
            EmbedIcon::Repeat => "repeat",
            EmbedIcon::Stop => "stop",
        };

        format!("{}{}.png", ICONS_BASE_URL, icon)
    }
}

impl From<FromInteractionUserCausedError> for CreateEmbed {
    fn from(from_interaction_user_caused_error: FromInteractionUserCausedError) -> Self {
        error(
            "Chyba",
            match from_interaction_user_caused_error {
                FromInteractionUserCausedError::NotInGuild => {
                    "Příkazy je možné použít pouze na serveru, v přímé konverzaci nikoli."
                        .to_owned()
                }
                FromInteractionUserCausedError::UserNotInVoiceChannel => {
                    "Pro ovládání je nutné se připojit do hlasového kanálu.".to_owned()
                }
            },
        )
    }
}

impl From<FromInteractionInternalError> for CreateEmbed {
    fn from(_: FromInteractionInternalError) -> Self {
        command_generic_error()
    }
}

impl From<command::UserCausedError> for CreateEmbed {
    fn from(user_caused_error: command::UserCausedError) -> Self {
        error(
            "Chyba",
            match user_caused_error {
                command::UserCausedError::UserInDifferentVoiceChannel => {
                    "Pro ovládání je nutné se připojit do hlasového kanálu, v němž se nachází bot."
                        .to_owned()
                }
                command::UserCausedError::CouldNotJoin(_) => {
                    "Nebylo možné připojit se do hlasového kanálu.".to_owned()
                }
                command::UserCausedError::NotPlaying => "Neprobíhá přehrávání.".to_owned(),
                command::UserCausedError::QueueMove(
                    player::QueueMoveIndexExceedsQueueLengthError(index),
                ) => {
                    format!("Fronta {}. pozici neobsahuje.", index + 1)
                }
                command::UserCausedError::Next(player::NextNoTrackError) => {
                    "Ve frontě se nenachází žádné další položky.".to_owned()
                }
                command::UserCausedError::Previous(player::PreviousNoTrackError) => {
                    "Ve frontě se nenachází žádné předchozí položky.".to_owned()
                }
            },
        )
    }
}

impl From<command::InternalError> for CreateEmbed {
    fn from(_: command::InternalError) -> Self {
        command_generic_error()
    }
}

impl From<command::Executed<'_>> for CreateEmbed {
    fn from(executed_command: command::Executed) -> Self {
        match executed_command {
            command::Executed::Play(None) => error(
                "Nenalezeno",
                "Dle zadaného textu nebyl nalezen žádný výsledek.",
            ),
            command::Executed::Play(Some(fetched_query)) => {
                let embed = base("Přidáno do fronty", EmbedIcon::Queue, fetched_query.title)
                    .url(fetched_query.url);
                match fetched_query.thumbnail_url {
                    None => embed,
                    Some(thumbnail_url) => embed.thumbnail(thumbnail_url),
                }
            }
            command::Executed::QueueView(queue) => {
                let index = queue.current_playing_track_index.unwrap_or(0);
                let start = (index as i32 - QUEUE_VIEW_MAX_TRACKS as i32).max(0) as usize;
                let end = (index + QUEUE_VIEW_MAX_TRACKS).min(queue.tracks.len());

                let mut queue_text = String::new();

                if start > 0 {
                    queue_text.push_str(format!("*předcházejících: {}*\n", start).as_str());
                }

                queue_text.push_str(
                    queue
                        .tracks
                        .iter()
                        .skip(start)
                        .take(end - start)
                        .enumerate()
                        .map(|(index, track)| {
                            let absolute_index = start + index;
                            format!(
                                "{}. [{}]({})",
                                absolute_index + 1,
                                match queue.current_playing_track_index {
                                    Some(current_playing_track_index)
                                        if absolute_index == current_playing_track_index =>
                                        format!("**{}** ", track.title),
                                    None | Some(_) => track.title.clone(),
                                },
                                track.youtube_url
                            )
                        })
                        .collect::<Vec<_>>()
                        .join("\n")
                        .as_str(),
                );

                if end != queue.tracks.len() {
                    queue_text.push_str(
                        format!("\n*následujících: {}*", queue.tracks.len() - end).as_str(),
                    );
                }

                base("Fronta", EmbedIcon::Queue, "Položky ve frontě:").description(queue_text)
            }
            command::Executed::QueueMove { index } => base(
                "Fronta",
                EmbedIcon::Queue,
                format!("Přehrávání posunuto na {}. pozici ve frontě.", index + 1),
            ),
            command::Executed::QueueRepeat(repeat) => base(
                "Ovládání",
                EmbedIcon::Repeat,
                format!(
                    "Opakované přehrávání fronty je {}.",
                    if repeat { "zapnuto" } else { "vypnuto" }
                )
                .as_str(),
            ),
            command::Executed::QueueShuffle => base(
                "Ovládání",
                EmbedIcon::Repeat,
                "Fronta byla náhodně zamíchána.",
            ),
            command::Executed::Next => {
                base("Ovládání", EmbedIcon::Next, "Přehrávání další položky.")
            }
            command::Executed::Previous => base(
                "Ovládání",
                EmbedIcon::Previous,
                "Přehrávání předchozí položky.",
            ),
            command::Executed::Pause => {
                base("Ovládání", EmbedIcon::Pause, "Přehrávání pozastaveno.")
            }
            command::Executed::Resume => {
                base("Ovládání", EmbedIcon::Resume, "Přehrávání pokračuje.")
            }
            command::Executed::Repeat(repeat) => base(
                "Ovládání",
                EmbedIcon::Repeat,
                format!(
                    "Opakované přehrávání aktuální položky je {}.",
                    if repeat { "zapnuto" } else { "vypnuto" }
                )
                .as_str(),
            ),
            command::Executed::Stop => base("Ovládání", EmbedIcon::Stop, "Přehrávání zastaveno."),
        }
    }
}
