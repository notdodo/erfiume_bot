"""
Module to call allertameteo.regione.emilia-romagna.it APIs.
"""

from __future__ import annotations

import asyncio
from dataclasses import asdict, dataclass
from decimal import Decimal

import httpx

from .logging import logger


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
    value: float

    def to_dict(self) -> dict[str, str | Decimal]:
        """
        Convert dataclass to dictionary, suitable for DynamoDB storage.
        """
        data = asdict(self)
        data["soglia1"] = Decimal(str(self.soglia1))
        data["soglia2"] = Decimal(str(self.soglia2))
        data["soglia3"] = Decimal(str(self.soglia3))
        data["value"] = (
            Decimal(str(self.value)) if self.value is not None else Decimal("-1.0")
        )

        return data


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


async def fetch_latest_time(client: httpx.AsyncClient) -> int:
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
        latest_time = int(data[0]["time"])
    except httpx.HTTPStatusError as e:
        logger.exception("Error fetching latest time: %s", e.response.status_code)
        raise
    except (KeyError, IndexError):
        logger.exception("Error fetching latest time: KeyError or IndexError")
        raise
    else:
        return latest_time


async def fetch_stations_data(client: httpx.AsyncClient, time: int) -> list[Stazione]:
    """
    Fetch the list of all stations from the latest update timestamp.
    """
    base_url = "https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-sensor-values-no-time?variabile=254,0,0/1,-,-,-/B13215&time={}"
    url = base_url.format(time)

    try:
        response = await client.get(url)
        response.raise_for_status()
        data = response.json()
        stazioni = []
        for stazione in data:
            if "time" not in stazione:
                if not stazione["value"]:
                    stazione["value"] = "-1.0"
                stazioni.append(
                    Stazione(
                        timestamp=time,
                        **stazione,
                    )
                )
    except httpx.HTTPStatusError as e:
        logger.exception("Error fetching stations data: %s", e.response.status_code)
        raise
    except (KeyError, IndexError):
        logger.exception("Error fetching stations data: KeyError or IndexError")
        raise
    else:
        return stazioni


async def fetch_time_series(
    client: httpx.AsyncClient,
    stazione: Stazione,
) -> list[Valore]:
    """
    Fetch additional data (time series) for a station.
    """
    url = f"https://allertameteo.regione.emilia-romagna.it/o/api/allerta/get-time-series/?stazione={stazione.idstazione}&variabile=254,0,0/1,-,-,-/B13215"
    try:
        response = await client.get(url)
        response.raise_for_status()
        valori = [Valore(**valore) for valore in response.json()]
    except httpx.HTTPStatusError:
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
    else:
        return valori


async def enrich_data(client: httpx.AsyncClient, stations: list[Stazione]) -> None:
    """Enrich station data with time series values."""
    tasks = [fetch_time_series(client, stazione) for stazione in stations]
    results = await asyncio.gather(*tasks)

    for stazione, dati in zip(stations, results):
        if dati:
            max_value = max(dati, key=lambda x: x.t)
            stazione.value = max_value.v
            stazione.timestamp = max_value.t
