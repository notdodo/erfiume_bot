"""
Handle bot intections with users.
"""

from __future__ import annotations

import json
import os
from datetime import datetime
from inspect import cleandoc
from typing import Any

import boto3
from telegram import Update
from telegram.ext import (
    Application,
    CommandHandler,
    ContextTypes,
    MessageHandler,
    filters,
)
from zoneinfo import ZoneInfo

from .logging import logger
from .storage import DynamoClient


async def fetch_bot_token() -> str:
    """
    Fetch the Telegram Bot token from AWS SM
    """
    environment = os.getenv("ENVIRONMENT", "staging")
    return boto3.client(
        service_name="secretsmanager",
        endpoint_url=("http://localhost:4566" if environment != "production" else None),
    ).get_secret_value(
        SecretId="telegram-bot-token",
    )["SecretString"]


async def start(update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
    """Send a message when the command /start is issued."""
    user = update.effective_user
    if update.message and user:
        await update.message.reply_html(
            rf"Ciao {user.mention_html()}! Scrivi il nome di una stazione da monitorare per iniziare (e.g. <b>Cesena</b>)",
        )


async def cesena(update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
    """Send a message when the command /cesena is issued."""
    db_client = await DynamoClient.create()
    stazione = await db_client.get_matching_station("Cesena")
    if stazione:
        timestamp = (
            datetime.fromtimestamp(
                int(stazione.timestamp) / 1000, tz=ZoneInfo("Europe/Rome")
            )
            .replace(tzinfo=None)
            .strftime("%d-%m-%Y %H:%M")
        )
        value = float(stazione.value)
        yellow = stazione.soglia1
        orange = stazione.soglia2
        red = stazione.soglia3
        if update.message:
            message = cleandoc(
                f"""Nome Stazione: {stazione.nomestaz}
                Valore: {value!r}
                Soglia Gialla: {yellow}
                Soglia Arancione: {orange}
                Soglia Rossa: {red}
                Ultimo rilevamento: {timestamp}"""
            )
            await update.message.reply_html(message)
    elif update.message:
        await update.message.reply_html(
            "Nessun stazione trovata!",
        )


async def handle_private_message(
    update: Update, context: ContextTypes.DEFAULT_TYPE
) -> None:
    """
    Handle messages writte from private chat to match a specific station
    """

    message = cleandoc(
        """Stazione non trovata!
        Inserisci esattamente il nome che vedi dalla pagina https://allertameteo.regione.emilia-romagna.it/livello-idrometrico
        Ad esempio 'Cesena', 'Lavino di Sopra' o 'S. Carlo'"""
    )
    if update.message and update.effective_chat and update.message.text:
        logger.info("Received private message: %s", update.message.text)
        db_client = await DynamoClient.create()
        stazione = await db_client.get_matching_station(update.message.text)
        if stazione:
            timestamp = (
                datetime.fromtimestamp(
                    int(stazione.timestamp) / 1000, tz=ZoneInfo("Europe/Rome")
                )
                .replace(tzinfo=None)
                .strftime("%d-%m-%Y %H:%M")
            )
            value = float(stazione.value)
            yellow = stazione.soglia1
            orange = stazione.soglia2
            red = stazione.soglia3
            message = cleandoc(
                f"""Nome Stazione: {stazione.nomestaz}
                Valore: {value!r}
                Soglia Gialla: {yellow}
                Soglia Arancione: {orange}
                Soglia Rossa: {red}
                Ultimo rilevamento: {timestamp}"""
            )
        await context.bot.send_message(
            chat_id=update.effective_chat.id,
            text=message,
        )


async def bot(event: dict[str, Any]) -> None:
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

    # Decode the incoming Telegram message
    if event.get("body"):
        update_dict = json.loads(event["body"])
        async with application:
            update = Update.de_json(update_dict, application.bot)
            await application.process_update(update)
