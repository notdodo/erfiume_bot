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

    zip_path: pulumi.Output[str]
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

    zip_lambda_package = local.Command(
        f"{resource_prefix}-lambda-create-zip",
        dir="../dist/lambda-package",
        create="zip -q -r ../lambda.zip .",
        archive_paths=["../dist/lambda.zip"],
        triggers=[
            pulumi.FileAsset("../app/poetry.lock"),
            pulumi.FileArchive("../dist/lambda-package"),
        ],
    )

    sha256_zip = local.Command(
        f"{resource_prefix}-sha256-lambda-zip",
        dir="../dist/",
        create="sha256sum lambda.zip | cut -d ' ' -f1",
        update="sha256sum lambda.zip | cut -d ' ' -f1",
        triggers=[
            pulumi.FileAsset("../app/poetry.lock"),
            zip_lambda_package.archive_paths,
        ],
        opts=pulumi.ResourceOptions(depends_on=[zip_lambda_package]),
    )

    return LambdaZip(
        zip_path=zip_lambda_package.archive_paths[0], zip_sha256=sha256_zip.stdout
    )
