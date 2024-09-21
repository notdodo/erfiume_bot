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
    AsyncDynamoDB,
    enrich_data,
    fetch_latest_time,
    fetch_stations_data,
    logger,
)


async def update() -> None:
    """
    Run main.
    """
    async with httpx.AsyncClient() as http_client, AsyncDynamoDB(
        table_name="Stazioni"
    ) as dynamo:
        try:
            latest_time = await fetch_latest_time(http_client)
            stations = await fetch_stations_data(http_client, latest_time)
            await enrich_data(http_client, stations)
            for stazione in stations:
                await dynamo.check_and_update_stazioni(stazione)
        except httpx.HTTPStatusError as e:
            logger.exception("HTTP error occurred: %d", e.response.status_code)
        except httpx.ConnectTimeout:
            logger.exception("Connection timeout")


async def main() -> None:
    """Run entry point periodic update task."""
    update_task = asyncio.create_task(update())
    await update_task


@logger.inject_lambda_context
def handler(_event: dict[str, Any], _context: LambdaContext) -> None:
    """
    AWS Lambda starting method
    """
    asyncio.run(main())


if __name__ == "__main__":
    asyncio.run(main())
