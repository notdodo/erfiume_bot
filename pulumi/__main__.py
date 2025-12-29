"""An AWS Python Pulumi program"""

import pulumi_cloudflare
from er_fiume import (
    Function,
    FunctionCPUArchitecture,
    FunctionRuntime,
    GenericRole,
    LambdaRole,
    Table,
    TableAttribute,
    TableAttributeType,
)
from pulumi_aws import apigatewayv2, dynamodb, lambda_, scheduler
from telegram_provider import (
    TelegramBot,
    TelegramBotCommand,
    TelegramBotCommandScopeType,
    TelegramBotCommandSet,
)

import pulumi

RESOURCES_PREFIX = "erfiume"
SYNC_MINUTES_RATE_NORMAL = 24 * 60  # Once a day
SYNC_MINUTES_RATE_MEDIUM = 2 * 60  # Every two hours
SYNC_MINUTES_RATE_EMERGENCY = 20
EMERGENCY = False
CUSTOM_DOMAIN_NAME = "erfiume.thedodo.xyz"

BOT_COMMANDS = [
    TelegramBotCommand(command="help", description="Visualizza la lista dei comandi"),
    TelegramBotCommand(
        command="info", description="Ottieni informazioni riguardanti il bot"
    ),
    TelegramBotCommand(command="start", description="Inizia ad interagire con il bot"),
    TelegramBotCommand(
        command="stazioni", description="Visualizza la lista delle stazioni disponibili"
    ),
    TelegramBotCommand(
        command="avvisami",
        description="Ricevi un avviso quando la soglia viene superata",
    ),
    TelegramBotCommand(
        command="lista_avvisi",
        description="Lista dei tuoi avvisi di superamento soglia",
    ),
    TelegramBotCommand(
        command="cambia_regione", description="Cambia la regione da monitorare"
    ),
    TelegramBotCommand(
        command="rimuovi_avviso", description="Rimuovi un avviso per la stazione"
    ),
]

er_stations_table = Table(
    name="EmiliaRomagna-Stations",
    hash_key="nomestaz",
    attributes=[TableAttribute(name="nomestaz", type=TableAttributeType.STRING)],
)

m_stations_table = Table(
    name="Marche-Stations",
    hash_key="nomestaz",
    attributes=[TableAttribute(name="nomestaz", type=TableAttributeType.STRING)],
)

alerts_table = Table(
    name="Alerts",
    hash_key="station",
    range_key="chat_id",
    attributes=[
        TableAttribute(name="station", type=TableAttributeType.STRING),
        TableAttribute(name="chat_id", type=TableAttributeType.NUMBER),
        TableAttribute(name="active", type=TableAttributeType.NUMBER),
    ],
    global_secondary_indexes=[
        dynamodb.TableGlobalSecondaryIndexArgs(
            name="chat_id-active-index",
            hash_key="chat_id",
            range_key="active",
            projection_type="INCLUDE",
            non_key_attributes=[
                "station",
                "threshold",
                "triggered_at",
                "triggered_value",
                "thread_id",
            ],
        ),
        dynamodb.TableGlobalSecondaryIndexArgs(
            name="station-active-index",
            hash_key="station",
            range_key="active",
            projection_type="INCLUDE",
            non_key_attributes=[
                "chat_id",
                "threshold",
                "triggered_at",
                "thread_id",
            ],
        ),
    ],
)

chats_table = Table(
    name="Chats",
    hash_key="chat_id",
    attributes=[TableAttribute(name="chat_id", type=TableAttributeType.NUMBER)],
)

fetcher_lambda = Function(
    name=f"{RESOURCES_PREFIX}-fetcher",
    role=LambdaRole(
        name=f"{RESOURCES_PREFIX}-fetcher",
        path=f"/{RESOURCES_PREFIX}/",
        permissions=[
            {
                "Effect": "Allow",
                "Actions": [
                    "dynamodb:GetItem",
                    "dynamodb:PutItem",
                    "dynamodb:Query",
                    "dynamodb:UpdateItem",
                ],
                # Query on GSIs requires index ARNs, not just the table ARN.
                "Resources": [
                    er_stations_table.arn,
                    m_stations_table.arn,
                    alerts_table.arn,
                    alerts_table.arn.apply(lambda arn: f"{arn}/index/*"),
                ],
            }
        ],
    ),
    code_runtime=FunctionRuntime.RUST,
    architecture=FunctionCPUArchitecture.ARM,
    memory=512,
    timeout=20,
    variables={
        "ALERTS_TABLE_NAME": alerts_table.table.name,
        "EMILIA_ROMAGNA_STATIONS_TABLE_NAME": er_stations_table.table.name,
        "ENVIRONMENT": pulumi.get_stack(),
        "MARCHE_STATIONS_TABLE_NAME": m_stations_table.table.name,
        "RUST_LOG": "info",
        "TELOXIDE_TOKEN": pulumi.Config().require_secret("telegram-bot-token"),
    },
)

bot_lambda = Function(
    name=f"{RESOURCES_PREFIX}-bot",
    role=LambdaRole(
        name=f"{RESOURCES_PREFIX}-bot",
        path=f"/{RESOURCES_PREFIX}/",
        permissions=[
            {
                "Effect": "Allow",
                "Actions": [
                    "dynamodb:DeleteItem",
                    "dynamodb:GetItem",
                    "dynamodb:PutItem",
                    "dynamodb:Query",
                    "dynamodb:Scan",
                    "dynamodb:UpdateItem",
                ],
                # Query on GSIs requires index ARNs, not just the table ARN.
                "Resources": [
                    er_stations_table.arn,
                    m_stations_table.arn,
                    alerts_table.arn,
                    alerts_table.arn.apply(lambda arn: f"{arn}/index/*"),
                    chats_table.arn,
                ],
            },
        ],
    ),
    code_runtime=FunctionRuntime.RUST,
    architecture=FunctionCPUArchitecture.ARM,
    memory=128,
    timeout=10,
    variables={
        "ALERTS_TABLE_NAME": alerts_table.table.name,
        "CHATS_TABLE_NAME": chats_table.table.name,
        "EMILIA_ROMAGNA_STATIONS_TABLE_NAME": er_stations_table.table.name,
        "ENVIRONMENT": pulumi.get_stack(),
        "MARCHE_STATIONS_TABLE_NAME": m_stations_table.table.name,
        "REGION_EMILIA_ROMAGNA_KEY": "emilia-romagna",
        "REGION_EMILIA_ROMAGNA_LABEL": "Emilia-Romagna",
        "REGION_MARCHE_KEY": "marche",
        "REGION_MARCHE_LABEL": "Marche",
        "RUST_LOG": "info",
        "STATIONS_SCAN_PAGE_SIZE": "50",
        "TELOXIDE_TOKEN": pulumi.Config().require_secret("telegram-bot-token"),
    },
)

scheduler.Schedule(
    f"{RESOURCES_PREFIX}-fetcher",
    name=f"{RESOURCES_PREFIX}-fetcher",
    flexible_time_window=scheduler.ScheduleFlexibleTimeWindowArgs(
        mode="FLEXIBLE",
        maximum_window_in_minutes=5,
    ),
    schedule_expression=f"rate({SYNC_MINUTES_RATE_EMERGENCY if EMERGENCY else SYNC_MINUTES_RATE_MEDIUM} minutes)",
    schedule_expression_timezone="Europe/Rome",
    target=scheduler.ScheduleTargetArgs(
        arn=fetcher_lambda.arn,
        role_arn=GenericRole(
            name=f"{RESOURCES_PREFIX}-fetcher-scheduler",
            path=f"/{RESOURCES_PREFIX}/",
            for_services=["scheduler.amazonaws.com"],
            permissions=[
                {
                    "Effect": "Allow",
                    "Actions": [
                        "lambda:InvokeFunction",
                    ],
                    "Resources": [fetcher_lambda.arn],
                }
            ],
        ).arn,
    ),
)

gw_domain_name = apigatewayv2.DomainName(
    f"{RESOURCES_PREFIX}-bot",
    domain_name=CUSTOM_DOMAIN_NAME,
    domain_name_configuration=apigatewayv2.DomainNameDomainNameConfigurationArgs(
        certificate_arn="arn:aws:acm:eu-west-1:841162699174:certificate/109ca827-8d70-4e11-8995-0b3dbdbd0510",
        endpoint_type="REGIONAL",
        security_policy="TLS_1_2",
    ),
)
bot_webhook_gw = apigatewayv2.Api(
    f"{RESOURCES_PREFIX}-webhook",
    protocol_type="HTTP",
    route_key="POST /erfiume_bot",
    target=bot_lambda.arn,
    disable_execute_api_endpoint=True,
)
lambda_.Permission(
    f"{RESOURCES_PREFIX}-lambda-bot-api-gateway",
    action="lambda:InvokeFunction",
    function=bot_lambda.arn,
    principal="apigateway.amazonaws.com",
    source_arn=bot_webhook_gw.execution_arn.apply(lambda arn: f"{arn}/*/*"),
)
gw_api_mapping = apigatewayv2.ApiMapping(
    f"{RESOURCES_PREFIX}-bot-domain-mapping",
    api_id=bot_webhook_gw.id,
    domain_name=gw_domain_name.domain_name,
    stage="$default",
)

pulumi_cloudflare.DnsRecord(
    f"{RESOURCES_PREFIX}-api-gw-cname",
    name="erfiume",
    type="CNAME",
    zone_id="cec5bf01afed114303a536c264a1f394",
    proxied=True,
    content=gw_domain_name.domain_name_configuration.target_domain_name,
    ttl=1,
)

telegram_authorization_token = pulumi.Config().require_secret(
    "telegram-authorization-token"
)
pulumi_cloudflare.Ruleset(
    f"{RESOURCES_PREFIX}-waf",
    zone_id="cec5bf01afed114303a536c264a1f394",
    name="erfiume-bot-check-authorization-header",
    description="erfiume_bot Block Invalid Authorization Header",
    kind="zone",
    phase="http_request_firewall_custom",
    rules=[
        pulumi_cloudflare.RulesetRuleArgs(
            action="block",
            expression="(cf.client.bot)",
            enabled=True,
        ),
        pulumi_cloudflare.RulesetRuleArgs(
            action="block",
            expression=telegram_authorization_token.apply(
                lambda header: f'(all(http.request.headers["x-telegram-bot-api-secret-token"][*] ne "{header}") and http.host eq "{CUSTOM_DOMAIN_NAME}")'  # noqa: E501
            ),
            enabled=True,
        ),
    ],
)

TelegramBot(
    f"{RESOURCES_PREFIX}-apigateway-registration",
    token=pulumi.Config().require_secret("telegram-bot-token"),
    authorization_token=telegram_authorization_token,
    react_on=[
        "message",
        "inline_query",
        "callback_query",
    ],
    url=f"https://{CUSTOM_DOMAIN_NAME}/erfiume_bot",
    command_sets=[
        TelegramBotCommandSet(scope=scope, commands=BOT_COMMANDS)
        for scope in [
            TelegramBotCommandScopeType.DEFAULT,
            TelegramBotCommandScopeType.ALL_PRIVATE_CHATS,
            TelegramBotCommandScopeType.ALL_GROUP_CHATS,
            TelegramBotCommandScopeType.ALL_CHAT_ADMINISTRATORS,
        ]
    ],
)
