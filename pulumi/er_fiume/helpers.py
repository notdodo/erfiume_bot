"""Helper functions for the library"""

from __future__ import annotations

import re

from pulumi import Resource, error, warn

# Regular expression for validating resource names
VALIDATING_REGEX = re.compile(r"^[a-zA-Z0-9_.-]+$")


def pulumi_error(message: str, resource: Resource | None = None) -> None:
    """
    Raise a ValueError with a pulumi message

    :param message: Message to display in the error
    :param resource: Linked resource that triggered the error
    :raises ValueError
    """
    error(message, resource)
    raise ValueError(message)


def pulumi_warning(message: str, resource: Resource | None = None) -> None:
    """
    Show a warning message during a pulumi action

    :param message: Message to display in the warning
    :param resource: Linked resource that triggered the warning
    """
    warn(message, resource)


def format_resource_name(name: str, resource: Resource | None = None) -> str | None:
    """
    Formats a string to be used as a Pulumi resource name.

    :param name: The proposed name of the resource.
    :param resource: The Pulumi resource for context in case of errors.
    :return: A formatted, valid Pulumi resource name.
    :raises NameError: If the name is invalid.
    """
    if VALIDATING_REGEX.match(name):
        return name.lower().replace(" ", "-").replace("_", "-")
    pulumi_error(
        f"Invalid resource name {name}. Only alphanumeric, '.', '-' and '_' are allowed.",
        resource,
    )
    return None
