use serenity::builder::{CreateEmbed, CreateEmbedAuthor, CreateEmbedFooter};
use serenity::model::Color;

const ICONS_BASE_URL: &str = "https://raw.githubusercontent.com/matous-volf/tranzistorak/main/icons/";

pub fn error(author_text: &str, title: &str) -> CreateEmbed {
    base(author_text, EmbedIcon::Error, title)
        .color(Color::RED)
}

pub fn base(author_text: &str, author_icon: EmbedIcon, title: &str) -> CreateEmbed {
    CreateEmbed::new()
        .footer(CreateEmbedFooter::new(format!("Tranzistorák v{}", crate::VERSION))
            .icon_url(EmbedIcon::Bot.url()))
        .author(CreateEmbedAuthor::new(author_text)
            .icon_url(author_icon.url()))
        .title(title)
}

pub enum EmbedIcon {
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
