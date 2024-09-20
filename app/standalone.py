"""
Main.
"""

from __future__ import annotations

import asyncio

import httpx

from erfiume import (
    DynamoClient,
    bot,
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
        while True:
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
            await asyncio.sleep(15 * 60)


async def main() -> None:
    """Run entry point for the bot and periodic update task."""
    update_task = asyncio.create_task(update())
    tg_task = asyncio.create_task(bot({}))
    await update_task
    await tg_task


if __name__ == "__main__":
    asyncio.run(main())
