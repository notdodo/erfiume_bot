"""Pulumi resources to create and configure Stations on AWS"""

from __future__ import annotations

from dataclasses import dataclass
from enum import StrEnum

import pulumi
from pulumi_aws import dynamodb

from .helpers import format_resource_name


class TableAttributeType(StrEnum):
    """Class TableAttributeType to use with table attribute"""

    STRING = "S"
    NUMBER = "N"
    BINARY = "B"
    BOOLEAN = "BOOL"
    NULL = "NULL"
    MAP = "M"
    LIST = "L"
    STRING_SET = "SS"
    NUMBER_SET = "NS"
    BINARY_SET = "BS"


@dataclass
class TableAttribute:
    """Class TableAttribute to define table attributes"""

    name: str
    type: TableAttributeType


class Stations(pulumi.ComponentResource):
    """
    A Pulumi custom resource to create a Stations table.

    :param name [str]: The name of the table to create.
    :param hash_key [str]: The hash key to create in the DynamoDB table.
    :param opts [pulumi.ResourceOptions | None]: Pulumi resource options for the custom resource.
    """

    def __init__(
        self,
        name: str,
        hash_key: str,
        attributes: list[TableAttribute] | None = None,
        ttl: str | None = None,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """
        Initialize the Stations class.
        """
        self.name = name
        self.attributes = attributes or []
        self.resource_name = f"{format_resource_name(name, self)}-table"
        super().__init__("notdodo:erfiume:Stations", self.name, {}, opts)

        ttl_attribute = (
            dynamodb.TableTtlArgs(
                attribute_name=ttl,
                enabled=True,
            )
            if ttl
            else None
        )

        self.table = dynamodb.Table(
            self.resource_name,
            name=self.name,
            billing_mode="PAY_PER_REQUEST",
            hash_key=hash_key,
            attributes=[
                dynamodb.TableAttributeArgs(
                    name=attr.name,
                    type=attr.type,
                )
                for attr in self.attributes
            ],
            ttl=ttl_attribute,
            opts=opts,
        )

        self.arn = self.table.arn
        self.register_outputs({"table": self.table, "arn": self.table.arn})
