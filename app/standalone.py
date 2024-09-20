"""
Main.
"""

from __future__ import annotations

import asyncio
import os

import boto3
import httpx

from erfiume import (
    DynamoClient,
    enrich_data,
    fetch_latest_time,
    fetch_stations_data,
    logger,
    tg_main,
)


async def fetch_bot_token() -> str:
    """
    Fetch the Telegram Bot token from AWS SM
    """
    environment = os.getenv("ENVIRONMENT", "production")
    return boto3.client(
        service_name="secretsmanager",
        endpoint_url=("http://localhost:4566" if environment != "production" else None),
    ).get_secret_value(
        SecretId="telegram-bot-token",
    )["SecretString"]


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
    tg_task = asyncio.create_task(tg_main(await fetch_bot_token()))
    await update_task
    await tg_task


if __name__ == "__main__":
    asyncio.run(main())
