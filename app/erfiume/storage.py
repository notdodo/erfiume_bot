"""
Module to handle interactions with storage (DynamoDB).
"""

from __future__ import annotations

import os
from typing import TYPE_CHECKING

import aioboto3
from boto3.dynamodb.conditions import Key
from botocore.exceptions import ClientError

from .apis import Stazione
from .logging import logger

if TYPE_CHECKING:
    from types_aiobotocore_dynamodb import DynamoDBServiceResource


class DynamoClient:
    """
    Asynchronous DynamoDB client that can be used for various operations on DynamoDB tables.
    This class is designed to be instantiated and used in other asynchronous methods.
    """

    def __init__(self, client: DynamoDBServiceResource):
        """
        Wrap the class in async context.
        """
        self.client = client

    @classmethod
    async def create(cls) -> DynamoClient:
        """
        Factory method to initialize the DynamoDB client.
        This method is asynchronous and sets up the connection based on environment.
        """
        environment = os.getenv("ENVIRONMENT", "staging")
        session = aioboto3.Session()

        async with session.resource(
            "dynamodb",
            endpoint_url=(
                "http://localhost:4566" if environment != "production" else None
            ),
        ) as client:
            return cls(client)

    async def check_and_update_stazioni(self, station: Stazione) -> None:
        """
        Check if the station data in DynamoDB is outdated compared to the given station object.
        If outdated or non-existent, update it with the new data.
        """
        try:
            table = await self.client.Table("Stazioni")
            response = await table.query(
                KeyConditionExpression=Key("idstazione").eq(station.idstazione),
            )

            # Get the latest timestamp from the DynamoDB response
            latest_timestamp = (
                int(response["Items"][0].get("timestamp"))  # type: ignore[arg-type]
                if response["Count"] > 0
                else 0
            )

            # If the provided station has newer data or the record doesn't exist, update DynamoDB
            if station.timestamp > latest_timestamp or response["Count"] == 0:
                logger.info(
                    "Updating data for station %s (%s)",
                    station.nomestaz,
                    station.idstazione,
                )
                await table.put_item(Item=station.to_dict())
        except ClientError as e:
            logger.exception(
                "Error while checking or updating station %s: %s", station.idstazione, e
            )
            raise
        except Exception as e:
            logger.exception("Unexpected error: %s", e)
            raise

    async def get_station(self, station_id: str) -> Stazione | None:
        """
        Retrieve a station from the DynamoDB table by its idstazione.
        Returns the station data as a dictionary, or None if not found.
        """
        try:
            table = await self.client.Table("Stazioni")
            response = await table.query(
                KeyConditionExpression=Key("idstazione").eq(station_id),
            )

            if response["Count"] > 0:
                return Stazione(**response["Items"][0])  # type: ignore[arg-type]
            logger.info("Station %s not found in DynamoDB.", station_id)
        except ClientError as e:
            logger.exception("Error while retrieving station %s: %s", station_id, e)
            raise
        except Exception as e:
            logger.exception("Unexpected error: %s", e)
            raise
        else:
            return None
