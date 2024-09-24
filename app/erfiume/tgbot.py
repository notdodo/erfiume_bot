"""
Handle bot intections with users.
"""

from __future__ import annotations

import json
import random
from inspect import cleandoc
from typing import TYPE_CHECKING, Any

from telegram import Update
from telegram.ext import (
    Application,
    CommandHandler,
    ContextTypes,
    MessageHandler,
    filters,
)

if TYPE_CHECKING:
    from aws_lambda_powertools.utilities.typing import LambdaContext

from aws_lambda_powertools.utilities import parameters

from .logging import logger
from .storage import AsyncDynamoDB

RANDOM_SEND_LINK = 10


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


def is_from_user(update: Update) -> bool:
    """Check if the update is from a real user."""
    return update.effective_user is not None


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


async def send_donation_link(
    update: Update, context: ContextTypes.DEFAULT_TYPE
) -> None:
    """Randomnly send a donation link."""
    if random.randint(1, 10) == RANDOM_SEND_LINK and update.effective_chat:  # noqa: S311
        message = """Contribuisci al progetto per mantenerlo attivo e sviluppare nuove funzionalità tramite una donazione: https://buymeacoffee.com/d0d0"""
        await context.bot.send_message(
            chat_id=update.effective_chat.id,
            text=message,
        )


async def send_project_link(update: Update, context: ContextTypes.DEFAULT_TYPE) -> None:
    """Randomnly send a link to the GitHub repository."""
    if random.randint(1, 50) == RANDOM_SEND_LINK and update.effective_chat:  # noqa: S311
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
    if (
        is_from_user(update)
        and is_from_private_chat(update)
        and update.effective_user
        and update.message
    ):
        user = update.effective_user
        message = rf"Ciao {user.mention_html()}! Scrivi il nome di una stazione da monitorare per iniziare (e.g. <b>Cesena</b> o <b>/S. Carlo</b>)"
        await update.message.reply_html(message)
    elif (
        is_from_user(update)
        and is_from_group(update)
        and update.effective_chat
        and update.message
    ):
        chat = update.effective_chat
        message = rf"Ciao {chat.title}! Per iniziare scrivete il nome di una stazione da monitorare (e.g. <b>/Cesena</b> o <b>/S. Carlo</b>)"
        await update.message.reply_html(message)


async def cesena(update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
    """Send a message when the command /cesena is issued."""
    async with AsyncDynamoDB(table_name="Stazioni") as dynamo:
        stazione = await dynamo.get_matching_station("Cesena")
        if stazione:
            if update.message:
                await update.message.reply_html(stazione.create_station_message())
        elif update.message:
            await update.message.reply_html(
                "Nessun stazione trovata!",
            )


async def handle_private_message(
    update: Update, context: ContextTypes.DEFAULT_TYPE
) -> None:
    """
    Handle messages written from private chat to match a specific station
    """

    message = cleandoc(
        """Stazione non trovata!
        Inserisci esattamente il nome che vedi dalla pagina https://allertameteo.regione.emilia-romagna.it/livello-idrometrico
        Ad esempio 'Cesena', 'Lavino di Sopra' o 'S. Carlo'"""
    )
    if update.message and update.effective_chat and update.message.text:
        logger.info("Received private message: %s", update.message.text)
        async with AsyncDynamoDB(table_name="Stazioni") as dynamo:
            stazione = await dynamo.get_matching_station(
                update.message.text.replace("/", "").strip()
            )
            if stazione and update.message:
                message = stazione.create_station_message()
            await context.bot.send_message(
                chat_id=update.effective_chat.id,
                text=message,
            )
            await send_random_messages(update, context)


async def handle_group_message(
    update: Update, context: ContextTypes.DEFAULT_TYPE
) -> None:
    """
    Handle messages writte from private chat to match a specific station
    """

    message = cleandoc(
        """Stazione non trovata!
        Inserisci esattamente il nome che vedi dalla pagina https://allertameteo.regione.emilia-romagna.it/livello-idrometrico
        Ad esempio '/Cesena', '/Lavino di Sopra' o '/S. Carlo'"""
    )
    if update.message and update.effective_chat and update.message.text:
        logger.info("Received group message: %s", update.message.text)
        async with AsyncDynamoDB(table_name="Stazioni") as dynamo:
            stazione = await dynamo.get_matching_station(
                update.message.text.replace("/", "").replace("erfiume_bot", "").strip()
            )
            if stazione and update.message:
                message = stazione.create_station_message()
            await context.bot.send_message(
                chat_id=update.effective_chat.id,
                text=message,
            )
            await send_random_messages(update, context)


# END HANDLERS


async def bot(event: dict[str, Any], _context: LambdaContext) -> None:
    """Run entry point for the bot"""
    application = Application.builder().token(await fetch_bot_token()).build()

    application.add_handler(CommandHandler("start", start))
    application.add_handler(CommandHandler("cesena", cesena))
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
        update_dict = json.loads(event["body"])
        async with application:
            update = Update.de_json(update_dict, application.bot)
            if update and has_joined_group(update):
                await start(update, None)
            await application.process_update(update)
