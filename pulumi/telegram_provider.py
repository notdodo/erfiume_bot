"""
Custom provider to register the API GW as Telegram webhook.
source: https://github.com/omerholz/chatbot-example/blob/serverless-telegram-bot/infra/bot_lambda.py
"""

from __future__ import annotations

from typing import TYPE_CHECKING, Any

import requests
from pulumi.dynamic import CreateResult, Resource, ResourceProvider

if TYPE_CHECKING:
    import pulumi


class _TelegramWebhookProvider(ResourceProvider):
    def create(self, props: dict[str, Any]) -> CreateResult:
        webhook_url = props["url"]
        token = props["token"]
        response = requests.post(
            f"https://api.telegram.org/bot{token}/setWebhook",
            json={"url": webhook_url},
            timeout=10,
        )
        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)
        return CreateResult(id_="-", outs={})


class Webhook(Resource):
    """
    Register a Telegram Webhook with a custom URL
    """

    def __init__(
        self, name: str, token: str | pulumi.Output[str], url: str | pulumi.Output[str]
    ) -> None:
        super().__init__(
            _TelegramWebhookProvider(), name, {"token": token, "url": url}, None
        )
