"""
Handle bot intections with users.
"""

from __future__ import annotations

from dataclasses import dataclass
from inspect import cleandoc
from random import randint
from typing import TYPE_CHECKING, Any

from telegram import Update
from telegram.ext import (
    Application,
    CommandHandler,
    ContextTypes,
    MessageHandler,
    filters,
)
from thefuzz import process  # type: ignore[import-untyped]

if TYPE_CHECKING:
    from aws_lambda_powertools.utilities.typing import LambdaContext

    from .apis import Stazione

from aws_lambda_powertools.utilities import parameters

from .apis import KNOWN_STATIONS
from .logging import logger
from .storage import AsyncDynamoDB

RANDOM_SEND_LINK = 10
FUZZ_SCORE_CUTOFF = 80


@dataclass
class Chat:
    """
    Telegram user
    """

    id: int
    title: str
    type: str

    def to_dict(self) -> dict[str, str | bool | int]:
        """Convert dataclass to dictionary, suitable for DynamoDB storage."""
        return {
            "id": self.id,
            "title": self.title,
            "type": self.type,
        }


@dataclass
class User:
    """
    Telegram user
    """

    id: int
    is_bot: bool
    first_name: str
    username: str
    language_code: str
    last_name: str | None = ""

    def to_dict(self) -> dict[str, str | bool | int]:
        """Convert dataclass to dictionary, suitable for DynamoDB storage."""
        return {
            "id": self.id,
            "is_bot": self.is_bot,
            "first_name": self.first_name,
            "last_name": "" if not self.last_name else self.last_name,
            "username": self.username,
            "language_code": self.language_code,
        }


# UTILS
async def fetch_bot_token() -> str:
    """
    Fetch the Telegram Bot token from AWS SM
    """
    return parameters.get_secret("telegram-bot-token")


def is_from_group(update: Update) -> bool:
    """Check if the update is from a group."""
    chat = update.effective_chat
    return chat is not None and chat.type in [
        "group",
        "supergroup",
    ]


def is_from_private_chat(update: Update) -> bool:
    """Check if the update is from a private chat with the bot."""
    return update.effective_chat is not None and update.effective_chat.type == "private"


def has_joined_group(update: Update) -> bool:
    """
    Handle event when the bot is add to a group chat
    """
    if is_from_group(update) and update.message and update.effective_chat:
        for new_user in update.message.new_chat_members:
            if new_user.username == "erfiume_bot":
                return True
    return False


async def fuzz_search_station(station_name: str) -> tuple[Stazione | None, str]:
    """Search for a station even if the name is not exactly correct."""
    fuzzy_query = process.extractOne(
        station_name, KNOWN_STATIONS, score_cutoff=FUZZ_SCORE_CUTOFF
    )
    if fuzzy_query:
        async with AsyncDynamoDB(table_name="Stazioni") as dynamo:
            return (
                await dynamo.get_matching_station(fuzzy_query[0]),
                fuzzy_query[0],
            )
    return None, ""


async def send_donation_link(
    update: Update, context: ContextTypes.DEFAULT_TYPE
) -> None:
    """Randomnly send a donation link."""
    if randint(1, 10) == RANDOM_SEND_LINK and update.effective_chat:  # noqa: S311
        message = """Contribuisci al progetto per mantenerlo attivo e sviluppare nuove funzionalità tramite una donazione: https://buymeacoffee.com/d0d0"""
        await context.bot.send_message(
            chat_id=update.effective_chat.id,
            text=message,
        )


async def send_project_link(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Randomnly send a link to the GitHub repository."""
    if randint(1, 50) == RANDOM_SEND_LINK and update.effective_chat:  # noqa: S311
        message = """Esplora o contribuisci al progetto open-source per sviluppare nuove funzionalità: https://github.com/notdodo/erfiume_bot"""
        await context.bot.send_message(
            chat_id=update.effective_chat.id,
            text=message,
        )


async def send_random_messages(
    update: Update, context: ContextTypes.DEFAULT_TYPE
) -> None:
    """Handle the send of random messages."""
    await send_donation_link(update, context)
    await send_project_link(update, context)


# END UTILS


# HANDLERS
async def start(update: Update, _: ContextTypes.DEFAULT_TYPE | None) -> None:
    """Send a message when the command /start is issued."""
    if update.effective_user and is_from_private_chat(update) and update.message:
        user = update.effective_user
        message = rf"Ciao {user.mention_html()}! Scrivi il nome di una stazione da monitorare per iniziare (e.g. <b>Cesena</b> o <b>/S. Carlo</b>) o cercane una con /stazioni"  # noqa: E501
        await update.message.reply_html(message)
    elif (
        update.effective_user
        and is_from_group(update)
        and update.effective_chat
        and update.message
    ):
        chat = update.effective_chat
        message = rf"Ciao {chat.title}! Per iniziare scrivete il nome di una stazione da monitorare (e.g. <b>/Cesena</b> o <b>/S. Carlo</b>) o cercane una con /stazioni"  # noqa: E501
        await update.message.reply_html(message)


async def cesena(update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
    """Send a message when the command /cesena is issued."""
    if update.effective_user:
        async with AsyncDynamoDB("Chats") as chats:
            is_throttled = await chats.check_throttled_user(
                User(**update.effective_user.to_dict())
            )
            if is_throttled > 0 and update.message:
                await update.message.reply_html(f"throttled for {is_throttled}!")
                return
    station, _match = await fuzz_search_station("Cesena")
    if update.message and station:
        await update.message.reply_html(station.create_station_message())
    elif update.message:
        await update.message.reply_html(
            "Nessun stazione trovata!",
        )


async def list_stations(update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
    """Send a message when the command /cesena is issued."""
    if update.message:
        await update.message.reply_html("\n".join(KNOWN_STATIONS))


async def info(update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
    """Send a message when the command /cesena is issued."""
    message = cleandoc(
        """
        Bot Telegram che permette di leggere i livelli idrometrici dei fiumi dell'Emilia Romagna.
        I dati idrometrici sono ottenuti dalle API messe a disposizione da allertameteo.regione.emilia-romagna.it.
        Il progetto è completamente open-source (https://github.com/notdodo/erfiume_bot).
        Per donazioni per mantenere il servizio attivo: buymeacoffee.com/d0d0

        Inizia con /start o /stazioni
        """
    )
    if update.message:
        await update.message.reply_html(message, disable_web_page_preview=True)


async def handle_private_message(
    update: Update, context: ContextTypes.DEFAULT_TYPE
) -> None:
    """
    Handle messages from private chat to match a specific station
    """

    message = cleandoc(
        """Stazione non trovata!
        Inserisci esattamente il nome che vedi dalla pagina https://allertameteo.regione.emilia-romagna.it/livello-idrometrico
        Ad esempio 'Cesena', 'Lavino di Sopra' o 'S. Carlo'.
        Se non sai quale cercare prova con /stazioni"""
    )
    if update.message and update.effective_chat and update.message.text:
        query = update.message.text.replace("/", "").strip()
        logger.info(query)
        station, match = await fuzz_search_station(query)
        if station:
            message = station.create_station_message()
            if query != match:
                message += (
                    "\nSe non è la stazione corretta prova ad affinare la ricerca."
                )
        await context.bot.send_message(
            chat_id=update.effective_chat.id,
            text=message,
        )
        await send_random_messages(update, context)


async def handle_group_message(
    update: Update, context: ContextTypes.DEFAULT_TYPE
) -> None:
    """
    Handle messages from groups to match a specific station
    """

    message = cleandoc(
        """Stazione non trovata!
        Inserisci esattamente il nome che vedi dalla pagina https://allertameteo.regione.emilia-romagna.it/livello-idrometrico
        Ad esempio '/Cesena', '/Lavino di Sopra' o '/S. Carlo'.
        Se non sai quale cercare prova con /stazioni"""
    )
    if update.message and update.effective_chat and update.message.text:
        query = update.message.text.replace("/", "").replace("erfiume_bot", "").strip()
        station, match = await fuzz_search_station(query)
        if station:
            message = station.create_station_message()
            if query != match:
                message += (
                    "\nSe non é la stazione corretta prova ad affinare la ricerca."
                )
        await context.bot.send_message(
            chat_id=update.effective_chat.id,
            text=message,
        )
        await send_random_messages(update, context)


# END HANDLERS


async def bot(event: dict[str, Any], _context: LambdaContext) -> None:
    """Run entry point for the bot"""
    application = Application.builder().token(await fetch_bot_token()).build()

    await application.bot.set_my_commands(
        commands=[
            ("/start", "Inizia ad interagire con il bot"),
            ("/stazioni", "Visualizza la lista delle stazioni disponibili"),
            ("/info", "Ottieni informazioni riguardanti il bot"),
        ]
    )
    application.add_handler(CommandHandler("start", start))
    application.add_handler(CommandHandler("cesena", cesena))
    application.add_handler(CommandHandler("stazioni", list_stations))
    application.add_handler(CommandHandler("info", info))
    application.add_handler(
        MessageHandler(
            filters.ChatType.PRIVATE & (filters.TEXT | filters.COMMAND),
            handle_private_message,
        )
    )
    application.add_handler(
        MessageHandler(
            (filters.ChatType.SUPERGROUP | filters.ChatType.GROUP)
            & (filters.COMMAND | filters.Mention("erfiume_bot")),
            handle_group_message,
        )
    )

    # Decode the incoming Telegram message
    if event.get("body"):
        import json

        update_dict = json.loads(event["body"])
        async with application:
            update = Update.de_json(update_dict, application.bot)
            if update and has_joined_group(update):
                await start(update, None)
            await application.process_update(update)
