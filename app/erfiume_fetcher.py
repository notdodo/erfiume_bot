"""
Lambda function to update the data from the APIs to the DynamoDB storage
"""

from __future__ import annotations

import asyncio
from typing import TYPE_CHECKING, Any

import httpx

if TYPE_CHECKING:
    from aws_lambda_powertools.utilities.typing import LambdaContext

from erfiume import (
    DynamoClient,
    enrich_data,
    fetch_latest_time,
    fetch_stations_data,
    logger,
)


async def update() -> None:
    """
    Run main.
    """
    db_client = await DynamoClient.create()
    async with httpx.AsyncClient() as http_client:
        try:
            latest_time = await fetch_latest_time(http_client)
            stations = await fetch_stations_data(http_client, latest_time)
            await enrich_data(http_client, stations)
            for stazione in stations:
                await db_client.check_and_update_stazioni(stazione)
        except httpx.HTTPStatusError as e:
            logger.exception("HTTP error occurred: %d", e.response.status_code)
        except httpx.ConnectTimeout:
            logger.exception("Connection timeout")


async def main() -> dict[str, Any]:
    """Run entry point periodic update task."""
    update_task = asyncio.create_task(update())
    await update_task
    return {}


def handler(_event: dict[str, Any], _context: LambdaContext) -> dict[str, Any]:
    """
    AWS Lambda starting method
    """
    asyncio.run(main())
