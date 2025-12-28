"""
Custom provider to register the API GW as Telegram webhook.
improved from: https://github.com/omerholz/chatbot-example/blob/serverless-telegram-bot/infra/bot_lambda.py
"""

from __future__ import annotations

from dataclasses import asdict
from enum import StrEnum
from typing import TYPE_CHECKING, Any, TypedDict, cast

import pulumi
import requests
from pulumi.dynamic import (
    CreateResult,
    DiffResult,
    ReadResult,
    Resource,
    ResourceProvider,
    UpdateResult,
)
from pydantic.dataclasses import dataclass

if TYPE_CHECKING:
    from pulumi import ResourceOptions


@dataclass
class TelegramBotInfo:
    """Answer from /getMe"""

    can_connect_to_business: bool
    can_join_groups: bool
    can_read_all_group_messages: bool
    first_name: str
    has_main_web_app: bool
    id: str
    is_bot: bool
    supports_inline_queries: bool
    username: str


@dataclass
class TelegramBotWebhookInfo:
    """Answer from /getWebhookInfo"""

    allowed_updates: list[str]
    has_custom_certificate: bool
    ip_address: str
    max_connections: int
    pending_update_count: int
    url: str
    last_error_date: int | None = None
    last_error_message: str | None = None


@dataclass
class TelegramOkResponse:
    """Response payload for boolean Telegram API calls."""

    ok: bool
    result: bool


@dataclass
class TelegramWebhookInfoResponse:
    """Response payload for getWebhookInfo."""

    ok: bool
    result: TelegramBotWebhookInfo


@dataclass
class TelegramBotInfoResponse:
    """Response payload for getMe."""

    ok: bool
    result: TelegramBotInfo


@dataclass
class TelegramBotCommandInfo:
    """Command entry from getMyCommands."""

    command: str
    description: str


@dataclass
class TelegramCommandsResponse:
    """Response payload for getMyCommands."""

    ok: bool
    result: list[TelegramBotCommandInfo]


@pulumi.input_type
class TelegramBotCommand:
    """Command definition for setMyCommands"""

    def __init__(self, command: pulumi.Input[str], description: pulumi.Input[str]):
        """Create a command definition."""
        pulumi.set(self, "command", command)
        pulumi.set(self, "description", description)

    @property
    @pulumi.getter
    def command(self) -> pulumi.Input[str]:
        """Command name."""
        return cast("pulumi.Input[str]", pulumi.get(self, "command"))

    @property
    @pulumi.getter
    def description(self) -> pulumi.Input[str]:
        """Command description."""
        return cast("pulumi.Input[str]", pulumi.get(self, "description"))


class TelegramBotCommandScopeType(StrEnum):
    """Supported command scope types for setMyCommands"""

    DEFAULT = "default"
    ALL_PRIVATE_CHATS = "all_private_chats"
    ALL_GROUP_CHATS = "all_group_chats"
    ALL_CHAT_ADMINISTRATORS = "all_chat_administrators"


@pulumi.input_type
class TelegramBotCommandSet:
    """Command list for a scope"""

    def __init__(
        self,
        commands: pulumi.Input[list[TelegramBotCommand]],
        scope: TelegramBotCommandScopeType | None = None,
    ):
        """Create a command set for a scope."""
        pulumi.set(self, "commands", commands)
        scope_payload = {"type": scope.value} if scope is not None else None
        pulumi.set(self, "scope", scope_payload)

    @property
    @pulumi.getter
    def commands(self) -> pulumi.Input[list[TelegramBotCommand]]:
        """Command list."""
        return cast(
            "pulumi.Input[list[TelegramBotCommand]]", pulumi.get(self, "commands")
        )

    @property
    @pulumi.getter
    def scope(self) -> pulumi.Input[dict[str, str] | None]:
        """Scope payload for Telegram."""
        return cast("pulumi.Input[dict[str, str] | None]", pulumi.get(self, "scope"))


class TelegramBotProps(TypedDict):
    """Typed props for the TelegramBot dynamic resource."""

    token: pulumi.Input[str]
    url: pulumi.Input[str]
    react_on: pulumi.Input[list[str]]
    authorization_token: pulumi.Input[str] | None
    command_sets: pulumi.Input[list[TelegramBotCommandSet]]


class _TelegramBotProvider(ResourceProvider):
    """Provider to interact with the Telegram Bot APIs."""

    def _set_webhook(
        self, token: str, url: str, react_on: list[str], secret_token: str | None
    ) -> TelegramOkResponse:
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
        return TelegramOkResponse(**response.json())

    def _delete_webhook(self, token: str) -> TelegramOkResponse:
        response = requests.post(
            f"https://api.telegram.org/bot{token}/deleteWebhook",
            timeout=10,
        )
        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)
        return TelegramOkResponse(**response.json())

    def _get_my_commands(
        self,
        token: str,
        scope: dict[str, str] | None,
    ) -> list[dict[str, str]]:
        response = requests.post(
            f"https://api.telegram.org/bot{token}/getMyCommands",
            json={"scope": scope} if scope else None,
            timeout=10,
        )
        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)
        payload = response.json()
        parsed = TelegramCommandsResponse(**payload)
        return [asdict(command) for command in parsed.result]

    def _set_my_commands(
        self,
        token: str,
        commands: list[dict[str, str]],
        scope: dict[str, str] | None,
    ) -> TelegramOkResponse:
        payload: dict[str, Any] = {"commands": commands}
        if scope:
            payload["scope"] = scope

        response = requests.post(
            f"https://api.telegram.org/bot{token}/setMyCommands",
            json=payload,
            timeout=10,
        )
        if response.status_code != requests.codes.OK:
            raise requests.RequestException(response.text)
        return TelegramOkResponse(**response.json())

    def _configure_commands(self, props: dict[str, Any]) -> None:
        token = props.get("token")
        if not token or not isinstance(token, str):
            return

        command_sets = props.get("command_sets") or []
        for command_set in command_sets:
            commands = command_set.get("commands")
            if not commands:
                continue
            scope = command_set.get("scope")
            self._set_my_commands(
                token,
                commands=commands,
                scope=scope if isinstance(scope, dict) else None,
            )

    def create(self, props: dict[str, Any]) -> CreateResult:
        self._set_webhook(
            props["token"],
            props["url"],
            props["react_on"],
            props.get("authorization_token"),
        )
        self._configure_commands(props)
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

        webhook_payload = TelegramWebhookInfoResponse(**response.json())
        webhook_info = webhook_payload.result
        bot_info_resp = requests.get(
            f"https://api.telegram.org/bot{token}/getMe", timeout=10
        )
        bot_payload = TelegramBotInfoResponse(**bot_info_resp.json())
        bot_info = bot_payload.result

        command_sets_actual = []
        command_sets = props.get("command_sets") or []
        for command_set in command_sets:
            scope = command_set.get("scope")
            scope_payload = scope if isinstance(scope, dict) else None
            commands = self._get_my_commands(token, scope_payload)
            command_sets_actual.append(
                {
                    "scope": scope_payload,
                    "commands": commands,
                }
            )

        props.update(
            {
                "webhook": asdict(webhook_info),
                "bot_info": asdict(bot_info),
                "command_sets_actual": command_sets_actual,
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

        if old.get("command_sets") != new.get("command_sets"):
            self._configure_commands(new)

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

        for k in [
            "url",
            "react_on",
            "authorization_token",
            "command_sets",
        ]:
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
    :param command_sets [list[TelegramBotCommandSet] | None]: Command definitions per scope
    :param opts [pulumi.ResourceOptions | None: Pulumi ResourceOptions
    """

    def __init__(
        self,
        name: str,
        token: str | pulumi.Output[str],
        url: str | pulumi.Output[str],
        react_on: list[str] | None = None,
        authorization_token: str | pulumi.Output[str] | None = None,
        command_sets: list[TelegramBotCommandSet] | None = None,
        opts: ResourceOptions | None = None,
    ):
        """Create the TelegramBot resource"""
        if not react_on:
            react_on = ["message", "inline_query"]

        if command_sets is None:
            command_sets = []

        props: TelegramBotProps = {
            "token": token,
            "url": url,
            "react_on": react_on,
            "authorization_token": authorization_token,
            "command_sets": command_sets,
        }
        super().__init__(
            _TelegramBotProvider(),
            name,
            props,
            opts,
        )
