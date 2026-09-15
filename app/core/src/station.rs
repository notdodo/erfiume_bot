use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Station {
    /// Observation time in Unix milliseconds.
    pub timestamp: Option<i64>,
    pub idstazione: String,
    pub ordinamento: i32,
    pub nomestaz: String,
    pub lon: String,
    pub lat: String,
    pub soglia1: f64,
    pub soglia2: f64,
    pub soglia3: f64,
    pub bacino: Option<String>,
    pub value: Option<f64>,
}
