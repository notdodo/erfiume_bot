"""
Handle bot intections with users.
"""

import asyncio
from datetime import datetime
from inspect import cleandoc

from telegram import Update
from telegram.ext import Application, CommandHandler, ContextTypes
from zoneinfo import ZoneInfo

from .storage import DynamoClient


async def start(update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
    """Send a message when the command /start is issued."""
    user = update.effective_user
    if update.message and user:
        await update.message.reply_html(
            rf"Ciao {user.mention_html()}!",
        )


async def cesena(update: Update, _: ContextTypes.DEFAULT_TYPE) -> None:
    """Send a message when the command /cesena is issued."""
    db_client = await DynamoClient.create()
    stazione = await db_client.get_station("-/1223505,4413971/spdsra")
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


async def bot(token: str) -> None:
    """Run entry point for the bot"""
    application = Application.builder().token(token).build()

    application.add_handler(CommandHandler("start", start))
    application.add_handler(CommandHandler("cesena", cesena))

    await application.initialize()
    await application.start()
    if application.updater:
        await application.updater.start_polling(allowed_updates=Update.ALL_TYPES)
    e = asyncio.Event()
    await e.wait()
