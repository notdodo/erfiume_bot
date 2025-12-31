use tracing::{error, info};

pub(crate) const TARGET: &str = "erfiume_fetcher";

#[derive(Clone, Default)]
pub(crate) struct Logger {
    chat_id: Option<i64>,
    station: Option<String>,
    threshold: Option<f64>,
    value: Option<f64>,
    error_text: Option<String>,
}

impl Logger {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn station(mut self, station: impl Into<String>) -> Self {
        self.station = Some(station.into());
        self
    }

    pub(crate) fn chat_id(mut self, chat_id: i64) -> Self {
        self.chat_id = Some(chat_id);
        self
    }

    pub(crate) fn threshold(mut self, threshold: f64) -> Self {
        self.threshold = Some(threshold);
        self
    }

    pub(crate) fn value(mut self, value: f64) -> Self {
        self.value = Some(value);
        self
    }

    pub(crate) fn error_text(mut self, error_text: impl Into<String>) -> Self {
        self.error_text = Some(error_text.into());
        self
    }

    pub(crate) fn info(&self, event: &'static str, message: &str) {
        let station = self.station.as_deref();
        info!(
            target: TARGET,
            event,
            chat_id = self.chat_id,
            station = station,
            threshold = self.threshold,
            value = self.value,
            error_text = ?self.error_text,
            "{}",
            message
        );
    }

    pub(crate) fn error<E: std::fmt::Debug>(&self, event: &'static str, err: &E, message: &str) {
        let station = self.station.as_deref();
        error!(
            target: TARGET,
            event,
            chat_id = self.chat_id,
            station = station,
            threshold = self.threshold,
            value = self.value,
            error_text = ?self.error_text,
            error = ?err,
            "{}",
            message
        );
    }
}
