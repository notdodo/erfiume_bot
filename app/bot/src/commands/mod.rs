use teloxide::utils::command::BotCommands;
pub(crate) mod handlers;
pub(crate) mod utils;
pub(crate) use handlers::{commands_handler, message_handler};

#[derive(BotCommands, Clone)]
#[command(rename_rule = "lowercase")]
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
    #[command(rename = "lista_avvisi")]
    ListaAvvisi,
    /// Rimuovi un avviso per la stazione
    #[command(rename = "rimuovi_avviso")]
    Rimuoviavviso(String),
    /// Rimuovi un avviso per la stazione (alias)
    #[command(rename = "rimuovi_avvisi")]
    Rimuoviavvisi(String),
}
