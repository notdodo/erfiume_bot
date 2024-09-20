"""An AWS Python Pulumi program"""

import pulumi
from pulumi_aws import (
    dynamodb,
    get_caller_identity,
    iam,
    lambda_,
    scheduler,
    secretsmanager,
)

from lambda_utils import create_lambda_zip

RESOURCES_PREFIX = "erfiume"

dynamodb.Table(
    f"{RESOURCES_PREFIX}-stazioni",
    name="Stazioni",
    billing_mode="PAY_PER_REQUEST",
    hash_key="idstazione",
    range_key="ordinamento",
    attributes=[
        dynamodb.TableAttributeArgs(
            name="idstazione",
            type="S",
        ),
        dynamodb.TableAttributeArgs(
            name="nomestaz",
            type="S",
        ),
        dynamodb.TableAttributeArgs(
            name="ordinamento",
            type="N",
        ),
    ],
    local_secondary_indexes=[
        dynamodb.TableLocalSecondaryIndexArgs(
            name="nomestaz",
            projection_type="KEYS_ONLY",
            range_key="nomestaz",
        )
    ],
)

secretsmanager.Secret(
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
                        ],
                        "Resources": [
                            f"arn:aws:dynamodb:eu-west-1:{get_caller_identity().account_id}:table/Stazioni"
                        ],
                    }
                ],
            ).json,
        )
    ],
)

lambda_zip = create_lambda_zip(RESOURCES_PREFIX)
fetcher_lambda = lambda_.Function(
    f"{RESOURCES_PREFIX}-fetcher",
    code=lambda_zip.zip_path,  # type: ignore[arg-type]
    name=f"{RESOURCES_PREFIX}-fetcher",
    role=fetcher_role.arn,
    handler="erfiume_fetcher.handler",
    source_code_hash=lambda_zip.zip_sha256,
    runtime=lambda_.Runtime.PYTHON3D12,
    environment={
        "variables": {
            "ENVIRONMENT": pulumi.get_stack(),
        },
    },
    timeout=60,
)

scheduler.Schedule(
    f"{RESOURCES_PREFIX}-fetcher",
    name=f"{RESOURCES_PREFIX}-fetcher",
    flexible_time_window=scheduler.ScheduleFlexibleTimeWindowArgs(
        mode="FLEXIBLE",
        maximum_window_in_minutes=5,
    ),
    schedule_expression="rate(25 minutes)",
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
