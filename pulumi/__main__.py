"""An AWS Python Pulumi program"""

import pulumi
import pulumi_cloudflare
from pulumi_aws import (
    apigatewayv2,
    cloudwatch,
    dynamodb,
    get_caller_identity,
    iam,
    lambda_,
    scheduler,
    secretsmanager,
)

from lambda_utils import create_lambda_layer, create_lambda_zip
from telegram_provider import Webhook

RESOURCES_PREFIX = "erfiume"
SYNC_MINUTES_RATE_NORMAL = 24 * 60  # Once a day
SYNC_MINUTES_RATE_EMERGENCY = 20
EMERGENCY = False
CUSTOM_DOMAIN_NAME = "erfiume.thedodo.xyz"

stazioni_table = dynamodb.Table(
    f"{RESOURCES_PREFIX}-stazioni",
    name="Stazioni",
    billing_mode="PAY_PER_REQUEST",
    hash_key="nomestaz",
    attributes=[
        dynamodb.TableAttributeArgs(
            name="nomestaz",
            type="S",
        ),
    ],
)

chats_table = dynamodb.Table(
    f"{RESOURCES_PREFIX}-users",
    name="Chats",
    billing_mode="PAY_PER_REQUEST",
    hash_key="chatid",
    attributes=[
        dynamodb.TableAttributeArgs(
            name="chatid",
            type="S",
        ),
    ],
    ttl=dynamodb.TableTtlArgs(
        attribute_name="ttl",
        enabled=True,
    ),
)

telegram_token_secret = secretsmanager.Secret(
    f"{RESOURCES_PREFIX}-telegram-bot-token",
    name="telegram-bot-token",
    description="The Telegram Bot token for erfiume_bot",
    recovery_window_in_days=7,
)

fetcher_role = iam.Role(
    f"{RESOURCES_PREFIX}-fetcher",
    name=f"{RESOURCES_PREFIX}-fetcher",
    assume_role_policy=iam.get_policy_document(
        statements=[
            {
                "Effect": "Allow",
                "Principals": [
                    {
                        "Type": "Service",
                        "Identifiers": ["lambda.amazonaws.com"],
                    }
                ],
                "Actions": ["sts:AssumeRole"],
            }
        ]
    ).json,
    managed_policy_arns=[
        "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
    ],
    inline_policies=[
        iam.RoleInlinePolicyArgs(
            name="FetcherRole",
            policy=iam.get_policy_document_output(
                statements=[
                    {
                        "Effect": "Allow",
                        "Actions": [
                            "dynamodb:PutItem",
                            "dynamodb:Query",
                            "dynamodb:UpdateItem",
                            "dynamodb:GetItem",
                        ],
                        "Resources": [stazioni_table.arn],
                    }
                ],
            ).json,
        )
    ],
)

bot_role = iam.Role(
    f"{RESOURCES_PREFIX}-bot",
    name=f"{RESOURCES_PREFIX}-bot",
    assume_role_policy=iam.get_policy_document(
        statements=[
            {
                "Effect": "Allow",
                "Principals": [
                    {
                        "Type": "Service",
                        "Identifiers": ["lambda.amazonaws.com"],
                    }
                ],
                "Actions": ["sts:AssumeRole"],
            }
        ]
    ).json,
    managed_policy_arns=[
        "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
    ],
    inline_policies=[
        iam.RoleInlinePolicyArgs(
            name="BotRole",
            policy=iam.get_policy_document_output(
                statements=[
                    {
                        "Effect": "Allow",
                        "Actions": [
                            "dynamodb:Query",
                            "dynamodb:GetItem",
                        ],
                        "Resources": [stazioni_table.arn, chats_table.arn],
                    },
                    {
                        "Effect": "Allow",
                        "Actions": [
                            "dynamodb:PutItem",
                        ],
                        "Resources": [chats_table.arn],
                    },
                    {
                        "Effect": "Allow",
                        "Actions": [
                            "secretsmanager:GetSecretValue",
                        ],
                        "Resources": [
                            telegram_token_secret.arn,
                        ],
                    },
                ],
            ).json,
        )
    ],
)

lambda_layer = create_lambda_layer(RESOURCES_PREFIX)
lambda_zip = create_lambda_zip(RESOURCES_PREFIX)
fetcher_lambda = lambda_.Function(
    f"{RESOURCES_PREFIX}-fetcher",
    code=lambda_zip.zip_path,
    name=f"{RESOURCES_PREFIX}-fetcher",
    role=fetcher_role.arn,
    handler="erfiume_fetcher.handler",
    source_code_hash=lambda_zip.zip_sha256,
    layers=[lambda_layer.arn],
    runtime=lambda_.Runtime.PYTHON3D12,
    environment={
        "variables": {
            "ENVIRONMENT": pulumi.get_stack(),
        },
    },
    memory_size=768,
    timeout=50,
)

bot_lambda = lambda_.Function(
    f"{RESOURCES_PREFIX}-bot",
    code=lambda_zip.zip_path,
    name=f"{RESOURCES_PREFIX}-bot",
    role=bot_role.arn,
    handler="erfiume_bot.handler",
    source_code_hash=lambda_zip.zip_sha256,
    layers=[lambda_layer.arn],
    runtime=lambda_.Runtime.PYTHON3D12,
    environment={
        "variables": {
            "ENVIRONMENT": pulumi.get_stack(),
        },
    },
    memory_size=768,
    timeout=10,
)

scheduler.Schedule(
    f"{RESOURCES_PREFIX}-fetcher",
    name=f"{RESOURCES_PREFIX}-fetcher",
    flexible_time_window=scheduler.ScheduleFlexibleTimeWindowArgs(
        mode="FLEXIBLE",
        maximum_window_in_minutes=5,
    ),
    schedule_expression=f"rate({SYNC_MINUTES_RATE_EMERGENCY if EMERGENCY else SYNC_MINUTES_RATE_NORMAL} minutes)",
    schedule_expression_timezone="Europe/Rome",
    target=scheduler.ScheduleTargetArgs(
        arn=fetcher_lambda.arn,
        role_arn=iam.Role(
            f"{RESOURCES_PREFIX}-fetcher-scheduler",
            name=f"{RESOURCES_PREFIX}-fetcher-scheduler",
            assume_role_policy=iam.get_policy_document(
                statements=[
                    {
                        "Effect": "Allow",
                        "Principals": [
                            {
                                "Type": "Service",
                                "Identifiers": ["scheduler.amazonaws.com"],
                            }
                        ],
                        "Actions": ["sts:AssumeRole"],
                        "conditions": [
                            {
                                "Test": "StringEquals",
                                "Variable": "aws:SourceAccount",
                                "Values": [f"{get_caller_identity().account_id}"],
                            }
                        ],
                    }
                ]
            ).json,
            inline_policies=[
                iam.RoleInlinePolicyArgs(
                    name="DynamoDBStazioniRW",
                    policy=iam.get_policy_document_output(
                        statements=[
                            {
                                "Effect": "Allow",
                                "Actions": [
                                    "lambda:InvokeFunction",
                                ],
                                "Resources": [fetcher_lambda.arn],
                            }
                        ],
                    ).json,
                )
            ],
        ).arn,
    ),
)

cloudwatch.LogGroup(
    f"{RESOURCES_PREFIX}-fetcher",
    log_group_class="STANDARD",
    name="/aws/lambda/erfiume-fetcher",
    retention_in_days=14,
)
cloudwatch.LogGroup(
    f"{RESOURCES_PREFIX}-bot",
    log_group_class="STANDARD",
    name="/aws/lambda/erfiume-bot",
    retention_in_days=14,
)

if pulumi.get_stack() == "production":
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
