pub(crate) mod search;
use chrono::{DateTime, TimeZone};
use chrono_tz::Europe::Rome;
use serde::Deserialize;

const UNKNOWN_VALUE: f64 = -9999.0;

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct Station {
    timestamp: i64,
    idstazione: String,
    ordinamento: i32,
    pub nomestaz: String,
    lon: String,
    lat: String,
    soglia1: f64,
    soglia2: f64,
    soglia3: f64,
    value: f64,
}

impl Station {
    pub fn create_station_message(&self) -> String {
        let timestamp_secs = self.timestamp / 1000;
        let naive_datetime = DateTime::from_timestamp(timestamp_secs, 0).unwrap();
        let datetime_in_tz: DateTime<chrono_tz::Tz> =
            Rome.from_utc_datetime(&naive_datetime.naive_utc());
        let timestamp_formatted = datetime_in_tz.format("%d-%m-%Y %H:%M").to_string();

        let value = self.value;

        let yellow = self.soglia1;
        let orange = self.soglia2;
        let red = self.soglia3;

        let mut alarm = "🔴";
        if value <= yellow {
            alarm = "🟢";
        } else if value > yellow && value <= orange {
            alarm = "🟡";
        } else if value >= orange && value <= red {
            alarm = "🟠";
        }

        let mut value_str = format!("{value:.2}");
        if value == UNKNOWN_VALUE {
            value_str = "non disponibile".to_string();
            alarm = "";
        }

        let yellow_str = format!("{yellow:.2}");
        let orange_str = format!("{orange:.2}");
        let red_str = format!("{red:.2}");

        format!(
            "Stazione: {}\nValore: {} {}\nSoglia Gialla: {}\nSoglia Arancione: {}\nSoglia Rossa: {}\nUltimo rilevamento: {}",
            self.nomestaz, value_str, alarm, yellow_str, orange_str, red_str, timestamp_formatted
        )
    }
}

pub fn stations() -> Vec<String> {
    let stations = vec![
        "Accursi Idice",
        "Alfonsine",
        "Alseno",
        "Anzola Ghironda",
        "Arcoveggio",
        "Ariano",
        "Battiferro Monte",
        "Bazzano",
        "Beccara Nuova Reno",
        "Bentivoglio",
        "Berceto Baganza",
        "Bevano Adriatica",
        "Bobbio",
        "Bomporto",
        "Bonconvento",
        "Bondanello",
        "Bondeno Panaro",
        "Borello",
        "Boretto",
        "Borgo Tossignano",
        "Borgo Visignolo",
        "Borgoforte",
        "Bova",
        "Brocchetti",
        "Burana",
        "Ca' de Caroli",
        "Cabanne",
        "Cadelbosco",
        "Calcara",
        "Calisese",
        "Camposanto",
        "Canonica Valle",
        "Capoponte",
        "Carignano Po",
        "Casale Monferrato Po",
        "Casalecchio canale",
        "Casalecchio chiusa",
        "Casalecchio tiro a volo",
        "Casalmaggiore",
        "Case Bonini",
        "Casei Gerola Po",
        "Casola Valsenio",
        "Casoni",
        "Cassa Crostolo SIAP",
        "Casse Espansione Enza monte",
        "Casse Espansione Enza SIAP",
        "Castel San Pietro",
        "Castelbolognese",
        "Castell'Arquato Canale",
        "Castell'Arquato",
        "Castelmaggiore",
        "Castenaso",
        "Castiglione",
        "Castrocaro",
        "Cavanella SIAP",
        "Cedogno",
        "Cento",
        "Centonara",
        "Cesena",
        "Cesenatico porto",
        "Chiavica Bastia Sillaro",
        "Chiavicone Idice",
        "Chiavicone Reno",
        "Ciriano",
        "Coccolia",
        "Codigoro",
        "Codrignano",
        "Colorno AIPO",
        "Compiano",
        "Conca Diga",
        "Concordia sulla Secchia",
        "Corniglio",
        "Correcchio canale",
        "Correcchio Sillaro",
        "Cotignola",
        "Cremona",
        "Crescentino Po",
        "Cusercoli Idro",
        "Diga di Ridracoli",
        "Dosso",
        "Due Tigli",
        "Faenza",
        "Fanano",
        "Farini",
        "Farneto",
        "Ferriere Idro",
        "Ficarolo",
        "Fidenza SIAP",
        "Fiorano",
        "Fiorenzuola d'Arda",
        "Firenzuola idro",
        "Fiscaglia Monte",
        "Fiscaglia Valle",
        "Fiumalbo",
        "Forcelli",
        "Forli'",
        "Fornovo SIAP",
        "Fornovo",
        "Foscaglia Panaro",
        "Fossalta",
        "Fusignano",
        "Gallo",
        "Gandazzolo Reno",
        "Gandazzolo Savena",
        "Gatta",
        "Gorzano",
        "Imola",
        "Impianto Forcelli Lavino",
        "Invaso",
        "Isola S.Antonio PO",
        "La Dozza",
        "Langhirano idro",
        "Lavino di Sopra",
        "Lavino di Sotto",
        "Linaro",
        "Loiano Ponte Savena",
        "Lonza",
        "Lugo SIAP",
        "Lugo",
        "Luretta",
        "Marradi",
        "Marsaglia",
        "Marzocchina",
        "Marzolara",
        "Massarolo",
        "Matellica",
        "Meldola",
        "Mercato Saraceno",
        "Mezzano",
        "Mignano Diga",
        "Modena Naviglio",
        "Modigliana",
        "Molato Diga Monte",
        "Montanaro",
        "Morciano di Romagna",
        "Mordano",
        "Navicello",
        "Noceto",
        "Ongina Po",
        "Ongina",
        "Opera Po",
        "Opera Reno Panfilia",
        "Ostia Parmense",
        "Palesio",
        "Paltrone Samoggia",
        "Parma cassa invaso CAE",
        "Parma Ovest",
        "Parma Ponte Nuovo",
        "Parma Ponte Verdi",
        "Parma S. Siro",
        "Piacenza",
        "Pianello Val Tidone idro",
        "Pianoro",
        "Pieve Cesato",
        "Pievepelago idro",
        "Pioppa",
        "Pizzocalvo",
        "Polesella SIAP",
        "Ponte Alto",
        "Ponte Bacchello",
        "Ponte Bastia",
        "Ponte Becca Po",
        "Ponte Braldo",
        "Ponte Calanca",
        "Ponte Cavola",
        "Ponte Dattaro",
        "Ponte degli Alpini",
        "Ponte dell'Olio",
        "Ponte Dolo",
        "Ponte Felisio",
        "Ponte Lamberti",
        "Ponte Locatello",
        "Ponte Messa",
        "Ponte Motta",
        "Ponte Nibbiano Tidoncello",
        "Ponte Nibbiano",
        "Ponte Ronca",
        "Ponte Samone",
        "Ponte Sant'Ambrogio",
        "Ponte Uso",
        "Ponte Val di Sasso",
        "Ponte Valenza Po",
        "Ponte Veggia",
        "Ponte Verucchio",
        "Ponte Vico",
        "Ponteceno",
        "Pontelagoscuro idrometro Boicelli",
        "Pontelagoscuro",
        "Pontenure",
        "Porretta Terme",
        "Portonovo",
        "Pracchia",
        "Pradella",
        "Puianello",
        "Quarto",
        "Ramiola",
        "Rasponi",
        "Ravone Torretta",
        "Ravone Via del Chiu",
        "Ravone",
        "Reda",
        "Riccardina",
        "Rimini Ausa",
        "Rimini SS16",
        "Rivalta RA",
        "Rivalta RE",
        "Rivergaro",
        "Rocca San Casciano",
        "Ronco",
        "Rossenna",
        "Rottofreno",
        "Rubiera casse monte",
        "Rubiera casse valle",
        "Rubiera SS9",
        "Rubiera Tresinaro",
        "S. Agata",
        "S. Antonio",
        "S. Bartolo",
        "S. Bernardino",
        "S. Carlo",
        "S. Cesario SIAP",
        "S. Donnino",
        "S. Ilario d'Enza",
        "S. Marco",
        "S. Maria Nova",
        "S. Ruffillo Savena",
        "S. Secondo",
        "S. Sofia",
        "S. Vittoria",
        "S. Zaccaria",
        "S. Zeno",
        "Saletto",
        "Saliceto",
        "Salsomaggiore sul Ghiara",
        "Salsominore",
        "Santarcangelo di Romagna",
        "Sarna",
        "Sasso Marconi",
        "Savignano",
        "Savio",
        "Secondo Salto",
        "Selvanizza",
        "Sermide",
        "Sesto Imolese",
        "Silla",
        "Sorbolo",
        "Sostegno Reno",
        "Spessa Po",
        "Spilamberto",
        "Strada Casale",
        "Suviana",
        "Tassone a Bagnolo",
        "Tebano",
        "Teodorano",
        "Toccalmatto",
        "Tornolo",
        "Trebbia Valsigiara",
        "Veggiola",
        "Vergato",
        "Vetto",
        "Vignola SIAP",
        "Vigoleno",
        "Vigolo Marchese",
        "Villanova",
    ];
    stations.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_station_message_with_unknown_value() {
        let station = Station {
            idstazione: "/id/".to_string(),
            timestamp: 1729454542656,
            ordinamento: 1,
            nomestaz: "Cesena".to_string(),
            lon: "lon".to_string(),
            lat: "lat".to_string(),
            soglia1: 1.0,
            soglia2: 2.0,
            soglia3: 3.0,
            value: UNKNOWN_VALUE,
        };
        let expected = "Stazione: Cesena\nValore: non disponibile \nSoglia Gialla: 1.00\nSoglia Arancione: 2.00\nSoglia Rossa: 3.00\nUltimo rilevamento: 20-10-2024 22:02".to_string();

        assert_eq!(station.create_station_message(), expected);
    }

    #[test]
    fn create_station_message() {
        let station = Station {
            idstazione: "/id/".to_string(),
            timestamp: 1729454542656,
            ordinamento: 1,
            nomestaz: "Cesena".to_string(),
            lon: "lon".to_string(),
            lat: "lat".to_string(),
            soglia1: 1.0,
            soglia2: 2.0,
            soglia3: 3.0,
            value: 2.2,
        };
        let expected = "Stazione: Cesena\nValore: 2.20 🟠\nSoglia Gialla: 1.00\nSoglia Arancione: 2.00\nSoglia Rossa: 3.00\nUltimo rilevamento: 20-10-2024 22:02".to_string();

        assert_eq!(station.create_station_message(), expected);
    }
}
