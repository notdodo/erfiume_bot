"""
erfium global library module
"""

from __future__ import annotations

from .apis import Stazione, Valore, enrich_data, fetch_latest_time, fetch_stations_data
from .logging import logger
from .storage import DynamoClient
from .tgbot import bot

__all__ = [
    "Stazione",
    "Valore",
    "DynamoClient",
    "enrich_data",
    "fetch_latest_time",
    "fetch_stations_data",
    "logger",
    "bot",
]
