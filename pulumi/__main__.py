"""An AWS Python Pulumi program"""

import pulumi
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
            name="DynamoDBStazioniRW",
            policy=iam.get_policy_document_output(
                statements=[
                    {
                        "Effect": "Allow",
                        "Actions": [
                            "dynamodb:PutItem",
                            "dynamodb:Query",
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
            name="DynamoSMReadOnly",
            policy=iam.get_policy_document_output(
                statements=[
                    {
                        "Effect": "Allow",
                        "Actions": [
                            "dynamodb:Query",
                            "dynamodb:GetItem",
                        ],
                        "Resources": [
                            f"arn:aws:dynamodb:eu-west-1:{get_caller_identity().account_id}:table/Stazioni"
                        ],
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
    memory_size=1024,
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
    memory_size=1024,
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
    bot_webhook_gw = apigatewayv2.Api(
        f"{RESOURCES_PREFIX}-webhook",
        protocol_type="HTTP",
        route_key="POST /erfiume_bot",
        target=bot_lambda.arn,
    )
    lambda_.Permission(
        f"{RESOURCES_PREFIX}-lambda-bot-api-gateway",
        action="lambda:InvokeFunction",
        function=bot_lambda.arn,
        principal="apigateway.amazonaws.com",
        source_arn=bot_webhook_gw.execution_arn.apply(lambda arn: f"{arn}/*/*"),
    )

    Webhook(
        f"{RESOURCES_PREFIX}-apigateway-registration",
        token=pulumi.Config().require_secret("telegram-bot-token"),
        url=bot_webhook_gw.api_endpoint.apply(lambda url: f"{url}/erfiume_bot"),
    )
