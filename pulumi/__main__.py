"""Pulumi entrypoint for the erfiume infrastructure"""

from pulumi_aws import apigatewayv2, dynamodb, lambda_, scheduler
from pulumi_cloudflare import DnsRecord, Ruleset, RulesetRuleArgs

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
from stack_config import StackConfig
from telegram_provider import (
    TelegramBot,
    TelegramBotCommand,
    TelegramBotCommandScopeType,
    TelegramBotCommandSet,
)

config = StackConfig.load()

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
    name=f"{config.resources_prefix}-fetcher",
    role=LambdaRole(
        name=f"{config.resources_prefix}-fetcher",
        path=f"/{config.resources_prefix}/",
        permissions=[
            {
                "Effect": "Allow",
                "Actions": [
                    "dynamodb:GetItem",
                    "dynamodb:PutItem",
                    "dynamodb:Query",
                    "dynamodb:UpdateItem",
                ],
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
    timeout=120,
    variables={
        "ALERTS_TABLE_NAME": alerts_table.table.name,
        "EMILIA_ROMAGNA_STATIONS_TABLE_NAME": er_stations_table.table.name,
        "ENVIRONMENT": config.environment,
        "MARCHE_STATIONS_TABLE_NAME": m_stations_table.table.name,
        "RUST_LOG": "info",
        "TELOXIDE_TOKEN": config.require_secret("telegram-bot-token"),
    },
)

bot_lambda = Function(
    name=f"{config.resources_prefix}-bot",
    role=LambdaRole(
        name=f"{config.resources_prefix}-bot",
        path=f"/{config.resources_prefix}/",
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
        "ENVIRONMENT": config.environment,
        "MARCHE_STATIONS_TABLE_NAME": m_stations_table.table.name,
        "REGION_EMILIA_ROMAGNA_KEY": "emilia-romagna",
        "REGION_EMILIA_ROMAGNA_LABEL": "Emilia-Romagna",
        "REGION_MARCHE_KEY": "marche",
        "REGION_MARCHE_LABEL": "Marche",
        "RUST_LOG": "info",
        "STATIONS_SCAN_PAGE_SIZE": config.stations_scan_page_size,
        "TELOXIDE_TOKEN": config.require_secret("telegram-bot-token"),
    },
)

scheduler.Schedule(
    f"{config.resources_prefix}-fetcher",
    name=f"{config.resources_prefix}-fetcher",
    flexible_time_window=scheduler.ScheduleFlexibleTimeWindowArgs(
        mode="FLEXIBLE",
        maximum_window_in_minutes=5,
    ),
    schedule_expression=f"rate({config.fetcher_rate_minutes} minutes)",
    schedule_expression_timezone="Europe/Rome",
    target=scheduler.ScheduleTargetArgs(
        arn=fetcher_lambda.arn,
        role_arn=GenericRole(
            name=f"{config.resources_prefix}-fetcher-scheduler",
            path=f"/{config.resources_prefix}/",
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
    f"{config.resources_prefix}-bot",
    domain_name=config.custom_domain_name,
    domain_name_configuration=apigatewayv2.DomainNameDomainNameConfigurationArgs(
        certificate_arn=config.certificate_arn,
        endpoint_type="REGIONAL",
        security_policy="TLS_1_2",
    ),
)
bot_webhook_gw = apigatewayv2.Api(
    f"{config.resources_prefix}-webhook",
    protocol_type="HTTP",
    route_key="POST /erfiume_bot",
    target=bot_lambda.arn,
    disable_execute_api_endpoint=True,
)
lambda_.Permission(
    f"{config.resources_prefix}-lambda-bot-api-gateway",
    action="lambda:InvokeFunction",
    function=bot_lambda.arn,
    principal="apigateway.amazonaws.com",
    source_arn=bot_webhook_gw.execution_arn.apply(lambda arn: f"{arn}/*/*"),
)
apigatewayv2.ApiMapping(
    f"{config.resources_prefix}-bot-domain-mapping",
    api_id=bot_webhook_gw.id,
    domain_name=gw_domain_name.domain_name,
    stage="$default",
)

DnsRecord(
    f"{config.resources_prefix}-api-gw-cname",
    name="erfiume",
    type="CNAME",
    zone_id=config.cloudflare_zone_id,
    proxied=True,
    content=gw_domain_name.domain_name_configuration.target_domain_name,
    ttl=1,
)

telegram_authorization_token = config.require_secret("telegram-authorization-token")
Ruleset(
    f"{config.resources_prefix}-waf",
    zone_id=config.cloudflare_zone_id,
    name="erfiume-bot-check-authorization-header",
    description="erfiume_bot Block Invalid Authorization Header",
    kind="zone",
    phase="http_request_firewall_custom",
    rules=[
        RulesetRuleArgs(
            action="block",
            expression="(cf.client.bot)",
            enabled=True,
        ),
        RulesetRuleArgs(
            action="block",
            expression=telegram_authorization_token.apply(
                lambda header: (
                    '(all(http.request.headers["x-telegram-bot-api-secret-token"][*] '
                    f'ne "{header}") and http.host eq "{config.custom_domain_name}")'
                )
            ),
            enabled=True,
        ),
    ],
)

TelegramBot(
    f"{config.resources_prefix}-apigateway-registration",
    token=config.require_secret("telegram-bot-token"),
    authorization_token=telegram_authorization_token,
    react_on=[
        "message",
        "inline_query",
        "callback_query",
    ],
    url=f"https://{config.custom_domain_name}/erfiume_bot",
    command_sets=[
        TelegramBotCommandSet(
            scope=scope,
            commands=[
                TelegramBotCommand(
                    command="help", description="Visualizza la lista dei comandi"
                ),
                TelegramBotCommand(
                    command="info",
                    description="Ottieni informazioni riguardanti il bot",
                ),
                TelegramBotCommand(
                    command="start", description="Inizia ad interagire con il bot"
                ),
                TelegramBotCommand(
                    command="stazioni",
                    description="Visualizza la lista delle stazioni disponibili",
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
                    command="cambia_regione",
                    description="Cambia la regione da monitorare",
                ),
                TelegramBotCommand(
                    command="rimuovi_avviso",
                    description="Rimuovi un avviso per la stazione",
                ),
            ],
        )
        for scope in [
            TelegramBotCommandScopeType.DEFAULT,
            TelegramBotCommandScopeType.ALL_PRIVATE_CHATS,
            TelegramBotCommandScopeType.ALL_GROUP_CHATS,
            TelegramBotCommandScopeType.ALL_CHAT_ADMINISTRATORS,
        ]
    ],
)
