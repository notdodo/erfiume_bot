"""Pulumi resources library"""

from .iam import GenericRole, LambdaRole
from .tables import Stations, TableAttribute, TableAttributeType

__all__ = [
    "GenericRole",
    "LambdaRole",
    "Stations",
    "TableAttribute",
    "TableAttributeType",
]
