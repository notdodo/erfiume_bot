"""
Lambda function to react to Telegram Bot messages
"""

from __future__ import annotations

import asyncio
import os

import boto3

from erfiume import tg_main


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


async def handler() -> None:
    """Run entry point for the bot and periodic update task."""
    token = await fetch_bot_token()
    tg_task = asyncio.create_task(tg_main(token))
    await tg_task


if __name__ == "__main__":
    asyncio.run(handler())
