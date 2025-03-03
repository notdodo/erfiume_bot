"""
Custom provider to register the API GW as Telegram webhook.
improved from: https://github.com/omerholz/chatbot-example/blob/serverless-telegram-bot/infra/bot_lambda.py
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import requests
from pulumi.dynamic import (
    CreateResult,
    ReadResult,
    Resource,
    ResourceProvider,
    UpdateResult,
)

if TYPE_CHECKING:
    import pulumi
    from pulumi import ResourceOptions


class _TelegramWebhookProvider(ResourceProvider):
    """Define how to interact with the Telegram API for the Webhook."""

    def create(self, props: dict[str, Any]) -> CreateResult:
        response = requests.post(
            f"https://api.telegram.org/bot{props['token']}/setWebhook",
            json={
                "url": props["url"],
                "allowed_updates": props["react_on"],
                "secret_token": props[".authorization_token"],
            },
            timeout=10,
        )
        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)
        return CreateResult(id_="-")

    def read(
        self,
        id: str,  # noqa: A002
        props: dict[str, Any],
    ) -> ReadResult:
        response = requests.get(
            f"https://api.telegram.org/bot{props['token']}/getWebhookInfo",
            timeout=10,
        )

        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)
        return ReadResult(id, response.json())

    def update(
        self,
        _id: str,
        _oldInputs: dict[str, Any],  # noqa: N803
        newInputs: dict[str, Any],  # noqa: N803
    ) -> UpdateResult:
        response = requests.post(
            f"https://api.telegram.org/bot{newInputs['token']}/setWebhook",
            json={
                "url": newInputs["url"],
                "allowed_updates": newInputs["react_on"],
                "secret_token": newInputs["authorization_token"],
            },
            timeout=10,
        )
        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)
        return UpdateResult(response.json())


class Webhook(Resource):
    """
    A Pulumi dynamic resource to create a Telegram Webhook

    :param name [str]: The name of the webhook to create.
    :param token [str | Output[str]]: Telegram token to use.
    :param url [str | Output[str]]: The url called by the webhook
    :param react_on [list[str] | None]: List actions that trigger the webhook.
    :param authorization_token [str | Output[str] | None]: pre-shared secret to authenticate telegram calls with target url
    :param opts [pulumi.ResourceOptions | None]: Pulumi resource options for the custom resource.
    """

    def __init__(  # noqa: PLR0913
        self,
        name: str,
        token: str | pulumi.Output[str],
        url: str | pulumi.Output[str],
        react_on: list[str] | None,
        authorization_token: str | pulumi.Output[str] | None = None,
        opts: ResourceOptions | None = None,
    ):
        """
        Initialize the Webhook class.
        """
        if not react_on:
            react_on = ["message", "inline_query"]
        super().__init__(
            _TelegramWebhookProvider(),
            name,
            {
                "token": token,
                "url": url,
                "react_on": react_on,
                "authorization_token": authorization_token,
            },
            opts,
        )
