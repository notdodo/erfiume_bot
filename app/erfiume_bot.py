"""
Lambda function to react to Telegram Bot messages
"""

from __future__ import annotations

import asyncio
import traceback
from typing import TYPE_CHECKING, Any

from erfiume import bot, logger

if TYPE_CHECKING:
    from aws_lambda_powertools.utilities.typing import LambdaContext


def handler(event: dict[str, Any], _context: LambdaContext) -> dict[str, Any]:
    """Run entry point for the bot."""
    logger.info("Received event: %s", event)
    try:
        asyncio.run(bot(event))
    except Exception as e:  # noqa: BLE001
        logger.error(f"An error occurred: {e!s}")
        logger.error(traceback.format_exc())
        return {"statusCode": 501}

    logger.info("Successfully processed event")
    return {"statusCode": 200}
