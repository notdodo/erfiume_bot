"""An AWS Python Pulumi program"""

import pulumi_cloudflare
from er_fiume import (
    Function,
    FunctionRuntime,
    GenericRole,
    LambdaRole,
    Stations,
    TableAttribute,
    TableAttributeType,
)
from pulumi_aws import apigatewayv2, cloudwatch, lambda_, scheduler
from telegram_provider import Webhook

import pulumi

RESOURCES_PREFIX = "erfiume"
SYNC_MINUTES_RATE_NORMAL = 24 * 60  # Once a day
SYNC_MINUTES_RATE_MEDIUM = 2 * 60  # Every two hours
SYNC_MINUTES_RATE_EMERGENCY = 20
EMERGENCY = False
CUSTOM_DOMAIN_NAME = "erfiume.thedodo.xyz"

er_stations_table = Stations(
    name="EmiliaRomagna-Stations",
    hash_key="nomestaz",
    attributes=[TableAttribute(name="nomestaz", type=TableAttributeType.STRING)],
)

m_stations_table = Stations(
    name="Marche-Stations",
    hash_key="nomestaz",
    attributes=[TableAttribute(name="nomestaz", type=TableAttributeType.STRING)],
)

chats_table = Stations(
    name="Chats",
    hash_key="id",
    attributes=[TableAttribute(name="id", type=TableAttributeType.NUMBER)],
    ttl="ttl",
)

fetcher_role = LambdaRole(
    name=f"{RESOURCES_PREFIX}-fetcher",
    path=f"/{RESOURCES_PREFIX}/",
    permissions=[
        {
            "Effect": "Allow",
            "Actions": [
                "dynamodb:PutItem",
                "dynamodb:Query",
                "dynamodb:UpdateItem",
                "dynamodb:GetItem",
            ],
            "Resources": [er_stations_table.arn, m_stations_table.arn],
        }
    ],
)

bot_role = LambdaRole(
    name=f"{RESOURCES_PREFIX}-bot",
    path=f"/{RESOURCES_PREFIX}/",
    permissions=[
        {
            "Effect": "Allow",
            "Actions": [
                "dynamodb:Query",
                "dynamodb:UpdateItem",
                "dynamodb:GetItem",
            ],
            "Resources": [er_stations_table.arn, m_stations_table.arn, chats_table.arn],
        },
        {
            "Effect": "Allow",
            "Actions": [
                "dynamodb:PutItem",
            ],
            "Resources": [chats_table.arn],
        },
    ],
)

fetcher_lambda = Function(
    name=f"{RESOURCES_PREFIX}-fetcher",
    role=fetcher_role,
    code_runtime=FunctionRuntime.RUST,
    memory=512,
    timeout=20,
    variables={
        "ENVIRONMENT": pulumi.get_stack(),
        "RUST_LOG": "info",
    },
)

bot_lambda = Function(
    name=f"{RESOURCES_PREFIX}-bot",
    role=bot_role,
    code_runtime=FunctionRuntime.RUST,
    memory=128,
    timeout=10,
    variables={
        "RUST_LOG": "info",
        "ENVIRONMENT": pulumi.get_stack(),
        "TELOXIDE_TOKEN": pulumi.Config().require_secret("telegram-bot-token"),
    },
)

scheduler_role = GenericRole(
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
        role_arn=scheduler_role.arn,
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

pulumi_cloudflare.Record(
    f"{RESOURCES_PREFIX}-api-gw-cname",
    name="erfiume",
    type="CNAME",
    zone_id="cec5bf01afed114303a536c264a1f394",
    proxied=True,
    content=gw_domain_name.domain_name_configuration.target_domain_name,
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

Webhook(
    f"{RESOURCES_PREFIX}-apigateway-registration",
    token=pulumi.Config().require_secret("telegram-bot-token"),
    authorization_token=telegram_authorization_token,
    react_on=[
        "message",
        "inline_query",
    ],
    url=f"https://{CUSTOM_DOMAIN_NAME}/erfiume_bot",
)
