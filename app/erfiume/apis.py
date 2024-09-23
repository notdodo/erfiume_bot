"""
Module to call allertameteo.regione.emilia-romagna.it APIs.
"""

from __future__ import annotations

import asyncio
from dataclasses import dataclass
from decimal import Decimal

import httpx

from .logging import logger

UNKNOWN_VALUE = -9999.0


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
        return int(data[0]["time"])
    except httpx.HTTPStatusError as e:
        logger.exception("Error fetching latest time: %s", e.response.status_code)
        raise
    except (KeyError, IndexError):
        logger.exception("Error fetching latest time: KeyError or IndexError")
        raise


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
    except httpx.HTTPStatusError as e:
        logger.exception("Error fetching stations data: %s", e.response.status_code)
        raise
    except (KeyError, IndexError):
        logger.exception("Error fetching stations data: KeyError or IndexError")
        raise


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
        return [Valore(**valore) for valore in response.json()]
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


async def enrich_data(client: httpx.AsyncClient, stations: list[Stazione]) -> None:
    """Enrich station data with time series values."""
    tasks = [fetch_time_series(client, stazione) for stazione in stations]
    results = await asyncio.gather(*tasks, return_exceptions=True)

    for stazione, dati in zip(stations, results):
        if isinstance(dati, BaseException):
            logger.error("Failed to fetch time series for station %s", stazione)
        else:
            max_value = max(dati, key=lambda x: x.t)
            stazione.value = max_value.v
            stazione.timestamp = max_value.t
