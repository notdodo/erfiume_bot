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
    "S. Zeno",
    "Spessa Po",
    "Parma S. Siro",
    "Mercato Saraceno",
    "Fiorenzuola d'Arda",
    "Fiscaglia Monte",
    "Navicello",
    "Camposanto",
    "Fidenza SIAP",
    "Codigoro",
    "Casoni",
    "Ponte Ronca",
    "Gallo",
    "Castenaso",
    "Correcchio Sillaro",
    "Beccara Nuova Reno",
    "Salsominore",
    "Vigoleno",
    "Lonza",
    "Morciano di Romagna",
    "Pievepelago idro",
    "Casse Espansione Enza monte",
    "S. Secondo",
    "Cassa Crostolo SIAP",
    "Tornolo",
    "Parma Ovest",
    "Rasponi",
    "Castel San Pietro",
    "Ponte dell'Olio",
    "Arcoveggio",
    "S. Sofia",
    "Lugo SIAP",
    "Pieve Cesato",
    "Cardinala Idice",
    "Ciriano",
    "Fossalta",
    "Fiorano",
    "Puianello",
    "Borgo Visignolo",
    "Cusercoli Idro",
    "Colorno AIPO",
    "Ficarolo",
    "Fusignano",
    "Foscaglia Panaro",
    "Teodorano",
    "Ponte Sant'Ambrogio",
    "Saletto",
    "Ponte Val di Sasso",
    "Case Bonini",
    "Capoponte",
    "Ponte Valenza Po",
    "Secondo Salto",
    "Ponte Vico",
    "Sermide",
    "Ponte Verucchio",
    "Battiferro Bypass",
    "Calcara",
    "Casalmaggiore",
    "Diga di Ridracoli",
    "Pontelagoscuro",
    "Fiscaglia Valle",
    "Molato Diga Monte",
    "S. Antonio",
    "Ponte Lamberti",
    "Linaro",
    "Montanaro",
    "Lugo",
    "Cremona",
    "Forcelli",
    "S. Agata",
    "Modena Naviglio",
    "Casalecchio canale",
    "Ponte Samone",
    "Bagnetto Reno",
    "Ponte Alto",
    "Ponte Messa",
    "Dosso",
    "Loiano Ponte Savena",
    "S. Carlo",
    "Ponte Braldo",
    "Vergato",
    "Mordano",
    "Castiglione",
    "Pracchia",
    "Ponte Becca Po",
    "Ongina",
    "Rivergaro",
    "Vignola SIAP",
    "S. Zaccaria",
    "Alseno",
    "Ramiola",
    "Savignano",
    "Strada Casale",
    "Rocca San Casciano",
    "S. Marco",
    "Bobbio",
    "Casola Valsenio",
    "Fornovo SIAP",
    "Pioppa",
    "Chiavicone Idice",
    "Ponte Veggia",
    "La Dozza",
    "Fanano",
    "Cadelbosco",
    "Sostegno Reno",
    "S. Bartolo",
    "Correcchio canale",
    "Canonica Valle",
    "Mezzano",
    "Saliceto",
    "Ponte Nibbiano",
    "Gandazzolo Reno",
    "S. Ruffillo Savena",
    "Farini",
    "Ostia Parmense",
    "Bova",
    "Palesio",
    "Modigliana",
    "Paltrone Samoggia",
    "Ponte Cavola",
    "Rimini Ausa",
    "Ponte Bacchello",
    "Sesto Imolese",
    "Pontenure",
    "Chiavica Bastia Sillaro",
    "Silla",
    "Ongina Po",
    "Sorbolo",
    "Isola S.Antonio PO",
    "Chiavicone Reno",
    "Parma Ponte Nuovo",
    "Rossenna",
    "Castellina di Soragna",
    "Pontelagoscuro idrometro Boicelli",
    "S. Vittoria",
    "Sarna",
    "Casale Monferrato Po",
    "Imola",
    "Mignano Diga",
    "Polesella SIAP",
    "Vetto",
    "Borello",
    "Ponte Calanca",
    "Rivalta RE",
    "Opera Reno Panfilia",
    "Tebano",
    "Parma cassa invaso CAE",
    "Bazzano",
    "Alfonsine",
    "Forli'",
    "Casalecchio tiro a volo",
    "Matellica",
    "Pianoro",
    "Porretta Terme",
    "Selvanizza",
    "Compiano",
    "Corniglio",
    "Lavino di Sotto",
    "Calisese",
    "Castell'Arquato Canale",
    "Bentivoglio",
    "Ponte Felisio",
    "S. Bernardino",
    "Ponte Dolo",
    "Borgoforte",
    "Luretta",
    "Marzocchina",
    "Trebbia Valsigiara",
    "S. Donnino",
    "Casse Espansione Enza SIAP",
    "Bondeno Panaro",
    "Carignano Po",
    "Borgo Tossignano",
    "Accursi Idice",
    "Isola Pescaroli SIAP",
    "Ravone Via del Chiu",
    "Anzola Ghironda",
    "Ponte Locatello",
    "Villanova",
    "Coccolia",
    "Sasso Marconi",
    "Santarcangelo di Romagna",
    "Ponte degli Alpini",
    "Centonara",
    "Bevano Adriatica",
    "Castrocaro",
    "Codrignano",
    "S. Ilario d'Enza",
    "Salsomaggiore sul Ghiara",
    "Berceto Baganza",
    "Veggiola",
    "Vigolo Marchese",
    "Cesena",
    "Castelmaggiore",
    "Casei Gerola Po",
    "Suviana",
    "Invaso",
    "Brocchetti",
    "Bonconvento",
    "Cento",
    "Burana",
    "Savio",
    "Fornovo",
    "Ponte Uso",
    "S. Cesario SIAP",
    "Piacenza",
    "Rubiera casse monte",
    "Pianello Val Tidone idro",
    "Conca Diga",
    "Cavanella SIAP",
    "Ponte Bastia",
    "Spilamberto",
    "Ariano",
    "S. Maria Nova",
    "Gatta",
    "Boretto",
    "Marsaglia",
    "Gorzano",
    "Rimini SS16",
    "Lavino di Sopra",
    "Castell'Arquato",
    "Cotignola",
    "Parma Ponte Verdi",
    "Ca' de Caroli",
    "Fiumalbo",
    "Rivalta RA",
    "Cedogno",
    "Ravone",
    "Castelbolognese",
    "Ponte Nibbiano Tidoncello",
    "Meldola",
    "Pizzocalvo",
    "Ponte Motta",
    "Quarto",
    "Ponteceno",
    "Noceto",
    "Gandazzolo Savena",
    "Crescentino Po",
    "Rubiera casse valle",
    "Monte Cerignone",
    "Impianto Forcelli Lavino",
    "Bondanello",
    "Firenzuola idro",
    "Ronco",
    "Rottofreno",
    "Ferriere Idro",
    "Bomporto",
    "Pradella",
    "Toccalmatto",
    "Langhirano idro",
    "Ponte Dattaro",
    "Marzolara",
    "Rubiera Tresinaro",
    "Massarolo",
    "Opera Po",
    "Concordia sulla Secchia",
    "Rubiera SS9",
    "Marradi",
    "Casalecchio chiusa",
    "Reda",
    "Cabanne",
    "Faenza",
    "Portonovo",
]
KNOWN_STATIONS.sort()


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
