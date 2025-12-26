"""Pulumi resources library"""

from .function import Function, FunctionCPUArchitecture, FunctionRuntime
from .iam import GenericRole, LambdaRole
from .tables import Table, TableAttribute, TableAttributeType

__all__ = [
    "Function",
    "FunctionCPUArchitecture",
    "FunctionRuntime",
    "GenericRole",
    "LambdaRole",
    "Table",
    "TableAttribute",
    "TableAttributeType",
]
