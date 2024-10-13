"""
erfium global library module
"""

from __future__ import annotations

from .apis import Stazione
from .logging import logger
from .storage import AsyncDynamoDB
from .tgbot import bot

__all__ = [
    "AsyncDynamoDB",
    "Stazione",
    "Valore",
    "logger",
    "bot",
]
