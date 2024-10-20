"""
Module to call allertameteo.regione.emilia-romagna.it APIs.
"""

from __future__ import annotations

from dataclasses import dataclass
from datetime import datetime
from decimal import Decimal
from inspect import cleandoc

from zoneinfo import ZoneInfo

UNKNOWN_VALUE = -9999.0

KNOWN_STATIONS = [
    "Accursi Idice",
    "Alfonsine",
    "Alseno",
    "Anzola Ghironda",
    "Arcoveggio",
    "Ariano",
    "Bagnetto Reno",
    "Battiferro Bypass",
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
    "Cardinala Idice",
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
    "Casse Espansione Enza SIAP",
    "Casse Espansione Enza monte",
    "Castel San Pietro",
    "Castelbolognese",
    "Castell'Arquato Canale",
    "Castell'Arquato",
    "Castellina di Soragna",
    "Castelmaggiore",
    "Castenaso",
    "Castiglione",
    "Castrocaro",
    "Cavanella SIAP",
    "Cedogno",
    "Cento",
    "Centonara",
    "Cesena",
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
    "Correcchio Sillaro",
    "Correcchio canale",
    "Cotignola",
    "Cremona",
    "Crescentino Po",
    "Cusercoli Idro",
    "Diga di Ridracoli",
    "Dosso",
    "Faenza",
    "Fanano",
    "Farini",
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
    "Isola Pescaroli SIAP",
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
    "Monte Cerignone",
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
    "Parma Ovest",
    "Parma Ponte Nuovo",
    "Parma Ponte Verdi",
    "Parma S. Siro",
    "Parma cassa invaso CAE",
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
    "Ponte degli Alpini",
    "Ponte dell'Olio",
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
    "Ravone Via del Chiu",
    "Ravone",
    "Reda",
    "Rimini Ausa",
    "Rimini SS16",
    "Rivalta RA",
    "Rivalta RE",
    "Rivergaro",
    "Rocca San Casciano",
    "Ronco",
    "Rossenna",
    "Rottofreno",
    "Rubiera SS9",
    "Rubiera Tresinaro",
    "Rubiera casse monte",
    "Rubiera casse valle",
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
]


@dataclass
class Stazione:
    """
    Stazione.
    """

    timestamp: int
    idstazione: str
    ordinamento: int
    nomestaz: str
    lon: str
    lat: str
    soglia1: float
    soglia2: float
    soglia3: float
    value: float | None

    def __post_init__(self) -> None:
        self.value = self.value or UNKNOWN_VALUE

    def to_dict(self) -> dict[str, str | Decimal | int]:
        """Convert dataclass to dictionary, suitable for DynamoDB storage."""
        return {
            "timestamp": self.timestamp,
            "idstazione": self.idstazione,
            "ordinamento": self.ordinamento,
            "nomestaz": self.nomestaz,
            "lon": self.lon,
            "lat": self.lat,
            "soglia1": Decimal(str(self.soglia1)),
            "soglia2": Decimal(str(self.soglia2)),
            "soglia3": Decimal(str(self.soglia3)),
            "value": Decimal(str(self.value)),
        }

    def create_station_message(self) -> str:
        """
        Create and format the answer from the bot.
        """
        timestamp = (
            datetime.fromtimestamp(
                int(self.timestamp) / 1000, tz=ZoneInfo("Europe/Rome")
            )
            .replace(tzinfo=None)
            .strftime("%d-%m-%Y %H:%M")
        )
        value = float(self.value)  # type: ignore [arg-type]
        yellow = self.soglia1
        orange = self.soglia2
        red = self.soglia3
        alarm = "🔴"
        if value <= yellow:
            alarm = "🟢"
        elif value > yellow and value <= orange:
            alarm = "🟡"
        elif value >= orange and value <= red:
            alarm = "🟠"

        if value == UNKNOWN_VALUE:
            value = "non disponibile"  # type: ignore[assignment]
            alarm = ""
        return cleandoc(
            f"""Stazione: {self.nomestaz}
                Valore: {value!r} {alarm}
                Soglia Gialla: {yellow}
                Soglia Arancione: {orange}
                Soglia Rossa: {red}
                Ultimo rilevamento: {timestamp}"""
        )
