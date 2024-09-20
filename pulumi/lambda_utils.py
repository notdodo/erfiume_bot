"""
Utils module to manage Lambda ZIP archives
"""

from __future__ import annotations

from dataclasses import dataclass

import pulumi
from pulumi_command import local


@dataclass
class LambdaZip:
    """
    Dataclass to store Lambda ZIP informations
    """

    zip_path: pulumi.FileArchive
    zip_sha256: pulumi.Output[str]


def create_lambda_zip(resource_prefix: str) -> LambdaZip:
    """
    Check changes on application folder to update the ZIP file for the lambda deployment
    """
    local.run(
        dir="../app/",
        command="poetry install --only main --sync",
        environment={"PYTHONUNBUFFERED": "1"},
    )

    local.run(
        dir="../",
        command="""
        mkdir -p ./dist/lambda-package; \
        cp -r ./app/.venv/lib/python*/site-packages/* ./dist/lambda-package/; \
        cp -r ./app/ ./dist/lambda-package/""",
    )

    local.run(
        dir="../dist/lambda-package",
        command=(
            "rm -rf .venv .mypy_cache .ruff_cache .env Makefile poetry.lock pyproject.toml standalone.py"
        ),
    )

    local.run(
        dir="../dist/lambda-package",
        command="zip -q -r ../lambda.zip .",
    )

    sha256_zip = local.Command(
        f"{resource_prefix}-sha256-lambda-zip",
        dir="../dist/",
        create="sha256sum lambda.zip | cut -d ' ' -f1",
        update="sha256sum lambda.zip | cut -d ' ' -f1",
        triggers=[
            pulumi.FileAsset("../app/poetry.lock"),
            pulumi.FileArchive("../dist/lambda.zip"),
        ],
    )

    return LambdaZip(
        zip_path=pulumi.FileArchive("../dist/lambda.zip"), zip_sha256=sha256_zip.stdout
    )
