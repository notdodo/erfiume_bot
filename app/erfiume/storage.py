"""
Module to handle interactions with storage (DynamoDB).
"""

from __future__ import annotations

import os
from decimal import Decimal
from typing import TYPE_CHECKING, Self

import aioboto3
import aioboto3.resources
from botocore.exceptions import ClientError

from .apis import Stazione
from .logging import logger

if TYPE_CHECKING:
    from types import TracebackType

UNKNOWN_VALUE = -9999.0


class AsyncDynamoDB:
    """
    Asynchronous DynamoDB client that can be used for various operations on DynamoDB tables.
    This class is designed to be instantiated and used in other asynchronous methods.
    """

    def __init__(self, table_name: str) -> None:
        environment = os.getenv("ENVIRONMENT", "staging")
        self.endpoint_url = (
            "http://localhost:4566" if environment != "production" else None
        )
        self.table_name = table_name

    async def __aenter__(self) -> Self:
        """Set up the client and table."""
        self.session = aioboto3.Session()
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
            )

            # Get the latest timestamp from the DynamoDB response
            latest_timestamp = (
                int(response["Item"].get("timestamp"))  # type: ignore[arg-type]
                if "Item" in response
                else 0
            )

            # If the provided station has newer data or the record doesn't exist, update DynamoDB
            if station.timestamp > latest_timestamp:
                logger.info("Updating data for station %s", station.nomestaz)
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
                logger.info("Creating data for station %s", station.nomestaz)
                await self.table.put_item(Item=station.to_dict())
        except ClientError as e:
            logger.exception(
                "Error while checking or updating station %s: %s", station.nomestaz, e
            )
            raise
        except Exception as e:
            logger.info("Stazione: %s", station)
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
            logger.info("Station %s not found in DynamoDB.", station_name)
        except ClientError as e:
            logger.exception("Error while retrieving station %s: %s", station_name, e)
            raise
        except Exception as e:
            logger.exception("Unexpected error: %s", e)
            raise
        else:
            return None
