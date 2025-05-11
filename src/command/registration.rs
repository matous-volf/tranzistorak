use serenity::all::{Command, CommandOptionType, Context, CreateCommand, CreateCommandOption};

pub(crate) async fn register_global_commands(context: &Context) -> serenity::Result<Vec<Command>> {
    Command::set_global_commands(&context.http, vec![
        CreateCommand::new("hrat")
            .description("Zařadí do fronty položku z odkazu nebo hledání.")
            .set_options(vec![
                CreateCommandOption::new(
                    CommandOptionType::String, "hledani", "odkaz nebo text k vyhledání",
                ).required(true)
            ])
            .dm_permission(false),
        CreateCommand::new("dalsi")
            .description("Přeskočí přehrávání na další pozici ve frontě.")
            .dm_permission(false),
        CreateCommand::new("predchozi")
            .description("Vrátí přehrávání na předchozí pozici ve frontě.")
            .dm_permission(false),
        CreateCommand::new("fronta")
            .description("Slouží k akcím s frontou přehrávání.")
            .set_options(vec![
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "zobrazit",
                    "Vypíše všechny položky ve frontě.",
                ),
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "posunout",
                    "Posune přehrávání na zadanou pozici ve frontě.",
                ).set_sub_options(vec![
                    CreateCommandOption::new(
                        CommandOptionType::Integer,
                        "pozice",
                        "pozice ve frontě k posunutí",
                    ).required(true).min_int_value(1),
                ]),
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "opakovat",
                    "Zapne nebo vypne opakování fronty.",
                ).set_sub_options(vec![
                    CreateCommandOption::new(
                        CommandOptionType::Boolean,
                        "zapnout",
                        "zda zapnout opakování",
                    ).required(true),
                ]),
                CreateCommandOption::new(
                    CommandOptionType::SubCommand,
                    "nahodne",
                    "Náhodně zamíchá frontu a začne přehrávat od první položky.",
                ),
            ])
            .dm_permission(false),
        CreateCommand::new("pauza")
            .description("Pozastaví přehrávání.")
            .dm_permission(false),
        CreateCommand::new("pokracovat")
            .description("Znovu spustí pozastavené přehrávání.")
            .dm_permission(false),
        CreateCommand::new("opakovat")
            .description("Zapne nebo vypne opakování aktuální položky.")
            .set_options(vec![
                CreateCommandOption::new(
                    CommandOptionType::Boolean,
                    "zapnout",
                    "zda zapnout opakování",
                ).required(true)
            ])
            .dm_permission(false),
        CreateCommand::new("stop")
            .description("Zastaví přehrávání, odstraní všechny položky ve frontě a opustí hlasový kanál.")
            .dm_permission(false),
    ]).await
}
