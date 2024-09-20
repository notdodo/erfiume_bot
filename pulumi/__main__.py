"""An AWS Python Pulumi program"""

import pulumi_archive as archive
from pulumi_aws import dynamodb, iam, lambda_, secretsmanager

import pulumi

RESOURCES_PREFIX = "erfiume"

dynamodb.Table(
    f"{RESOURCES_PREFIX}-stazioni",
    name="Stazioni",
    billing_mode="PAY_PER_REQUEST",
    hash_key="idstazione",
    attributes=[
        {
            "name": "idstazione",
            "type": "S",
        }
    ],
)

secretsmanager.Secret(
    f"{RESOURCES_PREFIX}-telegram-bot-token",
    name="telegram-bot-token",
    description="The Telegram Bot token for erfiume_bot",
    recovery_window_in_days=7,
)

assume_role = iam.get_policy_document(
    statements=[
        {
            "effect": "Allow",
            "principals": [
                {
                    "type": "Service",
                    "identifiers": ["lambda.amazonaws.com"],
                }
            ],
            "actions": ["sts:AssumeRole"],
        }
    ]
)
iam_for_lambda = iam.Role(
    "iam_for_lambda",
    name="iam_for_lambda",
    assume_role_policy=assume_role.json,
    managed_policy_arns=["arn:aws:iam::aws:policy/AdministratorAccess"],
)
lambda_fn = archive.get_file(
    type="zip",
    source_dir="../app/",
    output_path="lambda_function_payload.zip",
    excludes=["../app/.mypy_cache", "../app/.ruff_cache"],
)
test_lambda = lambda_.Function(
    "test_lambda",
    code=pulumi.FileArchive("lambda_function_payload.zip"),
    name="lambda_function_name",
    role=iam_for_lambda.arn,
    handler="erfiume_fetcher.handler",
    source_code_hash=lambda_fn.output_base64sha256,
    runtime=lambda_.Runtime.PYTHON3D12,
    environment={
        "variables": {
            "ENVIRONMENT": "staging",
        },
    },
)
