"""
Custom provider to register the API GW as Telegram webhook.
improved from: https://github.com/omerholz/chatbot-example/blob/serverless-telegram-bot/infra/bot_lambda.py
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import TYPE_CHECKING, Any

import requests
from pulumi.dynamic import (
    CreateResult,
    DiffResult,
    ReadResult,
    Resource,
    ResourceProvider,
    UpdateResult,
)

if TYPE_CHECKING:
    import pulumi
    from pulumi import ResourceOptions


@dataclass
class TelegramBotInfo:
    """Answer from /getMe"""

    id: str
    is_bot: bool
    first_name: str
    username: str
    can_join_grousp: bool
    can_read_all_group_messages: bool
    supports_inline_queries: bool
    can_connect_to_business: bool
    has_main_web_app: bool


@dataclass
class TelegramBotWebhookInfo:
    """Answer from /getWebhookInfo"""

    url: str
    has_custom_certificate: bool
    pending_update_count: int
    max_connections: int
    ip_address: str
    allowed_updates: list[str]


class _TelegramBotProvider(ResourceProvider):
    """Provider to interact with the Telegram Bot APIs."""

    def _set_webhook(
        self, token: str, url: str, react_on: list[str], secret_token: str | None
    ) -> Any:
        response = requests.post(
            f"https://api.telegram.org/bot{token}/setWebhook",
            json={
                "url": url,
                "allowed_updates": react_on,
                "secret_token": secret_token,
            },
            timeout=10,
        )
        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)
        return response.json()

    def _delete_webhook(self, token: str) -> Any:
        response = requests.post(
            f"https://api.telegram.org/bot{token}/deleteWebhook",
            timeout=10,
        )
        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)
        return response.json()

    def create(self, props: dict[str, Any]) -> CreateResult:
        self._set_webhook(
            props["token"],
            props["url"],
            props["react_on"],
            props.get("authorization_token"),
        )
        return CreateResult(id_="telegram-bot", outs=props)

    def read(
        self,
        id: str,  # noqa: A002
        props: dict[str, Any],
    ) -> ReadResult:
        token = props.get("token")
        if not token or not isinstance(token, str):
            # During preview, the token might not be available
            return ReadResult(id, props)

        response = requests.get(
            f"https://api.telegram.org/bot{token}/getWebhookInfo",
            timeout=10,
        )
        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)

        webhook_info = TelegramBotWebhookInfo(**response.json()["result"])
        bot_info_resp = requests.get(
            f"https://api.telegram.org/bot{token}/getMe", timeout=10
        )
        bot_info = TelegramBotInfo(
            **bot_info_resp.json()["result"] if bot_info_resp.ok else {}
        )

        props.update(
            {
                "webhook": webhook_info,
                "bot_info": bot_info,
            }
        )

        return ReadResult(id, props)

    def update(
        self,
        _id: str,
        old: dict[str, Any],
        new: dict[str, Any],
    ) -> UpdateResult:
        # Reconfigure webhook if changed
        if (
            old.get("url") != new.get("url")
            or old.get("react_on") != new.get("react_on")
            or old.get("authorization_token") != new.get("authorization_token")
        ):
            self._set_webhook(
                new["token"],
                new["url"],
                new["react_on"],
                new.get("authorization_token"),
            )

        return UpdateResult(outs=new)

    def delete(
        self,
        id: str,  # noqa: A002, ARG002
        props: dict[str, Any],
    ) -> None:
        self._delete_webhook(props["token"])

    def diff(
        self,
        id: str,  # noqa: A002, ARG002
        old: dict[str, Any],
        new: dict[str, Any],
    ) -> DiffResult:
        changes = False

        for k in ["url", "react_on", "authorization_token"]:
            if old.get(k) != new.get(k):
                changes = True

        return DiffResult(changes=changes, replaces=[])


class TelegramBot(Resource):
    """
    Pulumi dynamic resource to manage a Telegram Bot.

    :param name [str]: Resource name
    :param token [str | Output[str]]: Telegram bot token
    :param url [str | Output[str]]: Webhook URL
    :param react_on [list[str] | None]: List of updates the bot reacts to
    :param authorization_token [str | Output[str] | None]: Optional secret token for webhook verification
    :param opts [pulumi.ResourceOptions | None: Pulumi ResourceOptions
    """

    def __init__(
        self,
        name: str,
        token: str | pulumi.Output[str],
        url: str | pulumi.Output[str],
        react_on: list[str] | None = None,
        authorization_token: str | pulumi.Output[str] | None = None,
        opts: ResourceOptions | None = None,
    ):
        """Create the TelegramBot resource"""
        if not react_on:
            react_on = ["message", "inline_query"]

        super().__init__(
            _TelegramBotProvider(),
            name,
            {
                "token": token,
                "url": url,
                "react_on": react_on,
                "authorization_token": authorization_token,
            },
            opts,
        )
