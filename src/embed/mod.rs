use crate::{command, embed, player};
use serenity::builder::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter};
use serenity::model::Color;

const ICONS_BASE_URL: &str =
    "https://raw.githubusercontent.com/matous-volf/tranzistorak/main/icons/";
const QUEUE_VIEW_MAX_TRACKS: usize = 15;

pub(crate) fn error(author_text: impl Into<String>, title: impl Into<String>) -> CreateEmbed {
    base(author_text, EmbedIcon::Error, title).color(Color::RED)
}

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

impl From<command::ExecutionError> for CreateEmbed {
    fn from(_: command::ExecutionError) -> Self {
        error("Chyba", "Při vykonávání příkazu nastala chyba.")
    }
}

impl From<command::Error> for CreateEmbed {
    fn from(error: command::Error) -> Self {
        embed::error(
            "Chyba",
            match error {
                command::Error::NotInGuild => {
                    "Příkazy je možné použít pouze na serveru, v přímé konverzaci nikoli."
                        .to_owned()
                }
                command::Error::UserNotInVoiceChannel => {
                    "Pro ovládání je nutné se připojit do hlasového kanálu.".to_owned()
                }
                command::Error::UserInDifferentVoiceChannel => {
                    "Pro ovládání je nutné se připojit do hlasového kanálu, v němž se nachází bot."
                        .to_owned()
                }
                command::Error::CouldNotJoin(_) => {
                    "Nebylo možné připojit se do hlasového kanálu.".to_owned()
                }
                command::Error::NotPlaying => "Neprobíhá přehrávání.".to_owned(),
                command::Error::QueueMove(player::QueueMoveIndexExceedsQueueLengthError(index)) => {
                    format!("Fronta {}. pozici neobsahuje.", index + 1)
                }
                command::Error::Next(player::NextNoTrackError) => {
                    "Ve frontě se nenachází žádné další položky.".to_owned()
                }
                command::Error::Previous(player::PreviousNoTrackError) => {
                    "Ve frontě se nenachází žádné předchozí položky.".to_owned()
                }
            },
        )
    }
}

impl From<command::Executed<'_>> for CreateEmbed {
    fn from(executed: command::Executed) -> Self {
        match executed {
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
            command::Executed::QueueMove(index) => base(
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
