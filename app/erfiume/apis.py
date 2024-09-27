"""
Module to call allertameteo.regione.emilia-romagna.it APIs.
"""

from __future__ import annotations

from asyncio import gather as async_gather
from dataclasses import dataclass
from datetime import datetime
from decimal import Decimal
from inspect import cleandoc

from httpx import AsyncClient, HTTPStatusError
from zoneinfo import ZoneInfo

from .logging import logger

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


@dataclass
class Valore:
    """
    Single value from the sensor for a station.
    """

    t: int
    v: float

    def __post_init__(self) -> None:
        """
        Ensure that `t` is always converted to an int.
        """
        self.t = int(self.t)

    def to_dict(self) -> dict[str, int | Decimal]:
        """Convert dataclass to dictionary, suitable for DynamoDB storage."""
        return {"t": self.t, "v": Decimal(str(self.v))}


async def fetch_latest_time(client: AsyncClient) -> int:
    """
    Fetch the latest updated time.
    """
    base_time = "1726667100000"
    base_url = "https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-sensor-values-no-time?variabile=254,0,0/1,-,-,-/B13215&time={}"
    url = base_url.format(base_time)
    try:
        response = await client.get(url)
        response.raise_for_status()
        data = response.json()
        return int(data[0]["time"])
    except HTTPStatusError as e:
        logger.exception("Error fetching latest time: %s", e.response.status_code)
        raise
    except (KeyError, IndexError):
        logger.exception("Error fetching latest time: KeyError or IndexError")
        raise


async def fetch_stations_data(client: AsyncClient, time: int) -> list[Stazione]:
    """
    Fetch the list of all stations from the latest update timestamp.
    """
    base_url = "https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-sensor-values-no-time?variabile=254,0,0/1,-,-,-/B13215&time={}"
    url = base_url.format(time)

    try:
        response = await client.get(url)
        response.raise_for_status()
        data = response.json()
        return [
            Stazione(
                timestamp=time,
                **{
                    k: v if v is not None else UNKNOWN_VALUE
                    for k, v in stazione.items()
                },
            )
            for stazione in data
            if "time" not in stazione
        ]
    except HTTPStatusError as e:
        logger.exception("Error fetching stations data: %s", e.response.status_code)
        raise
    except (KeyError, IndexError):
        logger.exception("Error fetching stations data: KeyError or IndexError")
        raise


async def fetch_time_series(
    client: AsyncClient,
    stazione: Stazione,
) -> list[Valore]:
    """
    Fetch additional data (time series) for a station.
    """
    url = f"https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-time-series/?stazione={stazione.idstazione}&variabile=254,0,0/1,-,-,-/B13215"
    try:
        response = await client.get(url)
        response.raise_for_status()
        return [Valore(**valore) for valore in response.json()]
    except HTTPStatusError:
        logger.exception(
            "Error fetching time series for stagiong %s %s: %s",
            stazione.nomestaz,
            stazione.idstazione,
            response.status_code,
        )
        raise
    except KeyError:
        logger.exception(
            "Error fetching time series for station %s %s: KeyError",
            stazione.nomestaz,
            stazione.idstazione,
        )
        raise


async def enrich_data(client: AsyncClient, stations: list[Stazione]) -> None:
    """Enrich station data with time series values."""
    tasks = [fetch_time_series(client, stazione) for stazione in stations]
    results = await async_gather(*tasks, return_exceptions=True)

    for stazione, dati in zip(stations, results):
        if isinstance(dati, BaseException):
            logger.error("Failed to fetch time series for station %s", stazione)
        else:
            max_value = max(dati, key=lambda x: x.t)
            stazione.value = max_value.v
            stazione.timestamp = max_value.t
            stazione.timestamp = max_value.t
