use serenity::builder::CreateEmbed;
use serenity::utils::Color;

const ICONS_BASE_URL: &str = "https://matousvolf.cz/tranzistorak-icons/";

pub fn error(author_text: &str, title: &str) -> CreateEmbed {
    let mut embed = base(author_text, EmbedIcon::Error, title);
    embed.color(Color::RED);
    embed
}

pub fn base(author_text: &str, author_icon: EmbedIcon, title: &str) -> CreateEmbed {
    let mut embed = CreateEmbed::default();
    embed
        .footer(|footer| footer
            .text(format!("Tranzistorák v{}", crate::VERSION))
            .icon_url(EmbedIcon::Bot.url()))
        .author(|author|
            author.name(author_text)
                .icon_url(author_icon.url()))
        .title(title);

    embed
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
