"""
erfium global library module
"""

from __future__ import annotations

from .apis import Stazione, Valore, enrich_data, fetch_latest_time, fetch_stations_data
from .logging import logger
from .storage import AsyncDynamoDB
from .tgbot import bot

__all__ = [
    "AsyncDynamoDB",
    "Stazione",
    "Valore",
    "enrich_data",
    "fetch_latest_time",
    "fetch_stations_data",
    "logger",
    "bot",
]
