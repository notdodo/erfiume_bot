# erfiume_bot <a href="https://www.buymeacoffee.com/d0d0" target="_blank"><img src="https://www.buymeacoffee.com/assets/img/custom_images/yellow_img.png" alt="Buy Me A Coffee" style="height: 25px !important;width: 130px !important;box-shadow: 0px 3px 2px 0px rgba(190, 190, 190, 0.5) !important;-webkit-box-shadow: 0px 3px 2px 0px rgba(190, 190, 190, 0.5) !important;" ></a>

[![Pulumi Up](https://github.com/notdodo/erfiume_bot/actions/workflows/pulumi-up.yml/badge.svg)](https://github.com/notdodo/erfiume_bot/actions/workflows/pulumi-up.yml)
[![Deploy Bot](https://github.com/notdodo/erfiume_bot/actions/workflows/bot-deploy.yml/badge.svg)](https://github.com/notdodo/erfiume_bot/actions/workflows/bot-deploy.yml)

![Alt](https://repobeats.axiom.co/api/embed/35afc992da7617e96b55ad9a8765b0f06b50e3be.svg "Repobeats analytics image")

<p align="center">
  <img src="https://github.com/user-attachments/assets/58bb5033-87e0-4794-99f7-ddc1a3fd65b4" width="400px"/>
</p>

## Introduction

[`@erfiume_bot`](https://t.me/erfiume_bot) is a Telegram bot that fetches the water levels of the rivers in Emilia-Romagna and Marche. Data is retrieved from the [Allerta Meteo Emilia Romagna](https://allertameteo.regione.emilia-romagna.it/) APIs and from the [Protezione Civile Marche](http://app.protezionecivile.marche.it/sol/annaliidro2/index.sol?lang=it) portal, then periodically stored/updated in DynamoDB.
The bot can be used in both private or group chats, responding to specific station names or commands or alerting when a threshold is reached.

![](https://github.com/user-attachments/assets/f5bc07c1-fb6c-48be-b871-a9d6dd4aae82)

## Basic commands:

- `/start`
- `/info`
- `/stazioni`
- `/cambia_regione`
- `/<station_name>` where `<station_name>` is one of the stations reported on [Livello Idrometrico](https://allertameteo.regione.emilia-romagna.it/livello-idrometrico) or on the [Protezione Civile Marche portal](http://app.protezionecivile.marche.it/sol/annaliidro2/index.sol?lang=it)
- `/avvisami <station_name> <threshold>`
- `/lista_avvisi`
- `/rimuovi_avviso`

## Architecture

The bot consists of two main components:

1. **User interaction**: the code in `./app/bot` is triggered by a Telegram webhook that starts an AWS Lambda function when a user interacts with the bot.
2. **Stations data update**: the code in `./app/fetcher` runs on a Lambda function via an EventBridge scheduler, updating the data from the stations. This data is then used by the bot to answer to messages or to trigger alert notifications.

![](./assets/erfiume.png)

### Main technologies:

- **Pulumi** for IaC
- **AWS Lambda** for main code execution environment
- **DynamoDB** for storing data
- **EventBridge Scheduler** to trigger the syncing from Allerta Meteo
- **teloxide** to manage Telegram inputs
- **tokio** for managing asynchronous tasks in the Lambdas
- **Levenshtein Distance** for performing fuzzy search on station names from user input

## Features

### Telegram Bot (`./app/bot`)

The bot responds to Telegram messages via the main Lambda function and can:

- respond to any message or command (`/`) in private chats
- react when added to a group or supergroup
- process any command (`/`) in group or supergroup chats
- set up and manage alerts to notify the user of the reaching of a threshold

What it cannot do:

- read non-command messages in groups or supergroups
- provide [inline support](https://telegram.org/blog/inline-bots)
- support mentions

### Data fetcher (`./app/fetcher`)

This Lambda function is scheduled to fetch data from the APIs on [Allerta Meteo Emilia Romagna](https://allertameteo.regione.emilia-romagna.it/) and from the [Protezione Civile Marche](http://app.protezionecivile.marche.it/sol/annaliidro2/index.sol?lang=it) portal, then update or create station data in DynamoDB. A station refers to a sensor installed on a bridge or along a river that monitors water levels. Using the newly fetched information, the function also sends alerts to users who are subscribed to specific threshold notifications.

The Lambda is scheduled to run once every 2 hours in "normal" mode, but in "emergency" mode, it can be set to update data every 20 minutes or less.

## Repository Structure

- **app/**: Contains the bot and fetcher code as single Rust workspace.
- **pulumi/**: IaC for the AWS infrastructure.

## Disclaimer

The accuracy and reliability of the data is entirely dependent on [Allerta Meteo Emilia Romagna](https://allertameteo.regione.emilia-romagna.it/) and [Protezione Civile Marche](http://app.protezionecivile.marche.it/sol/annaliidro2/index.sol?lang=it): `erfiume_bot` just collects and displays the available data from these sources.
