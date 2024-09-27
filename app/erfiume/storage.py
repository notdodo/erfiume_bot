"""
Module to handle interactions with storage (DynamoDB).
"""

from __future__ import annotations

from datetime import datetime, timedelta
from decimal import Decimal
from os import getenv
from typing import TYPE_CHECKING, Self
from zoneinfo import ZoneInfo

from aioboto3 import Session
from botocore.exceptions import ClientError

from .apis import Stazione
from .logging import logger

if TYPE_CHECKING:
    from types import TracebackType

    from .tgbot import Chat, User

UNKNOWN_VALUE = -9999.0
THROTTLING_THRESHOLD = 5
THROTTLING_TIME = 15


class AsyncDynamoDB:
    """
    Asynchronous DynamoDB client that can be used for various operations on DynamoDB tables.
    This class is designed to be instantiated and used in other asynchronous methods.
    """

    def __init__(self, table_name: str) -> None:
        environment = getenv("ENVIRONMENT", "staging")
        self.endpoint_url = (
            "http://localhost:4566" if environment != "production" else None
        )
        self.table_name = table_name

    async def __aenter__(self) -> Self:
        """Set up the client and table."""
        self.session = Session()
        self.dynamodb = await self.session.resource(
            service_name="dynamodb",
            endpoint_url=self.endpoint_url,
        ).__aenter__()
        self.table = await self.dynamodb.Table(self.table_name)
        return self

    async def __aexit__(
        self,
        exc_type: type[Exception] | None,  # noqa: PYI036
        exc_val: Exception | None,  # noqa: PYI036
        exc_tb: TracebackType | None,
    ) -> None:
        """Close the client on exit."""
        await self.dynamodb.__aexit__(exc_type, exc_val, exc_tb)

    async def check_and_update_stazioni(self, station: Stazione) -> None:
        """
        Check if the station data in DynamoDB is outdated compared to the given station object.
        If outdated or non-existent, update it with the new data.
        """
        try:
            response = await self.table.get_item(
                Key={"nomestaz": station.nomestaz},
                ProjectionExpression="timestamp",
            )

            # Get the latest timestamp from the DynamoDB response
            latest_timestamp = (
                int(response["Item"].get("timestamp"))  # type: ignore[arg-type]
                if "Item" in response
                else 0
            )

            # If the provided station has newer data or the record doesn't exist, update DynamoDB
            if station.timestamp > latest_timestamp:
                await self.table.update_item(
                    Key={"nomestaz": station.nomestaz},
                    UpdateExpression="SET #ts = :new_timestamp, #vl = :new_value",
                    ExpressionAttributeValues={
                        ":new_timestamp": station.timestamp,
                        ":new_value": (
                            Decimal(str(station.value))
                            if station.value is not None
                            else Decimal(str(UNKNOWN_VALUE))
                        ),
                    },
                    ExpressionAttributeNames={
                        "#ts": "timestamp",
                        "#vl": "value",
                    },
                )
            elif not response["Item"]:
                await self.table.put_item(Item=station.to_dict())
        except ClientError as e:
            logger.exception(
                "Error while checking or updating station %s: %s", station.nomestaz, e
            )
            raise
        except Exception as e:
            logger.exception("Unexpected error: %s", e)
            raise

    async def get_matching_station(self, station_name: str) -> Stazione | None:
        """
        Retrieve a station from the DynamoDB table by its idstazione.
        Returns the station data as a dictionary, or None if not found.
        """
        try:
            stazione = await self.table.get_item(
                Key={"nomestaz": station_name},
            )

            if "Item" in stazione:
                return Stazione(**stazione["Item"])  # type: ignore[arg-type]
        except ClientError as e:
            logger.exception("Error while retrieving station %s: %s", station_name, e)
            raise
        except Exception as e:
            logger.exception("Unexpected error: %s", e)
            raise
        else:
            return None

    async def check_throttled_user(self, user: User | Chat) -> int:
        """
        Check if the a chat or a user must be throttled due to high-volume of requests.
        """
        try:
            response = await self.table.get_item(
                Key={"id": user.id},
                ProjectionExpression="#cnt ,#tl",
                ExpressionAttributeNames={"#cnt": "count", "#tl": "ttl"},
            )
            now = datetime.now(tz=ZoneInfo("Europe/Rome"))
            now_with_wait_time = int(
                (now + timedelta(minutes=THROTTLING_TIME)).timestamp()
            )
            if "Item" in response:
                if int(response["Item"].get("count", 0)) > THROTTLING_THRESHOLD:  # type: ignore[arg-type]
                    await self.table.update_item(
                        Key={"id": user.id},
                        UpdateExpression="set #tl = :new_ttl",
                        ExpressionAttributeNames={
                            "#tl": "ttl",
                        },
                        ExpressionAttributeValues={":new_ttl": now_with_wait_time},
                    )
                    wait_time = int(
                        (
                            datetime.fromtimestamp(
                                int(response["Item"].get("ttl")),  # type: ignore[arg-type]
                                tz=ZoneInfo("Europe/Rome"),
                            )
                            - now
                        ).total_seconds()
                    )
                    logger.info("Throttled %s for %s", user, wait_time)
                    return wait_time
                await self.table.update_item(
                    Key={"id": user.id},
                    ExpressionAttributeValues={
                        ":inc": 1,
                        ":new_ttl": now_with_wait_time,
                    },
                    UpdateExpression="ADD #cnt :inc SET #tl = :new_ttl",
                    ExpressionAttributeNames={
                        "#cnt": "count",
                        "#tl": "ttl",
                    },
                )
            else:
                user_info = user.to_dict()
                user_info.update({"count": 1, "ttl": now_with_wait_time})
                await self.table.put_item(Item=user_info)
        except ClientError as e:
            logger.exception("Error while adding chat or user %s: %s", user, e)
            raise
        except Exception as e:
            logger.info("Stazione: %s", user)
            logger.exception("Unexpected error: %s", e)
            raise
        else:
            return 0
