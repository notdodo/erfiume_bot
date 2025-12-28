use teloxide::utils::command::BotCommands;
pub(crate) mod handlers;
pub(crate) mod utils;
pub(crate) use handlers::{commands_handler, message_handler};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "snake_case")]
pub(crate) enum Command {
    /// Visualizza la lista dei comandi
    Help,
    /// Ottieni informazioni riguardanti il bot
    Info,
    /// Inizia ad interagire con il bot
    Start,
    /// Visualizza la lista delle stazioni disponibili
    Stazioni,
    /// Ricevi un avviso quando la soglia viene superata
    Avvisami(String),
    /// Lista dei tuoi avvisi di superamento soglia
    #[command(alias = "lista_avviso", hide_aliases)]
    ListaAvvisi,
    /// Rimuovi un avviso per la stazione
    #[command(alias = "rimuovi_avvisi", hide_aliases)]
    RimuoviAvviso(String),
}
