"""
Utils module to manage Lambda ZIP archives
"""

from __future__ import annotations

from dataclasses import dataclass

import pulumi
import pulumi_aws as aws
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
    dist_folder = "lambda-package"
    output_file = "lambda.zip"
    local.run(
        dir="../",
        command=f"""
        mkdir -p ./dist/{dist_folder}; \
        cp -r -p ./app/ ./dist/{dist_folder}/""",
    )

    local.run(
        dir=f"../dist/{dist_folder}",
        command=f"""rm -rf .venv .mypy_cache .ruff_cache .env Makefile poetry.lock pyproject.toml standalone.py; \
            zip -q -r -D -X -9 -A ../{output_file} .""",
    )

    sha256_zip = local.Command(
        f"{resource_prefix}-sha256-lambda-zip",
        dir="../dist/",
        create=f"sha256sum {output_file} | cut -d ' ' -f1",
        update=f"sha256sum {output_file} | cut -d ' ' -f1",
        triggers=[
            pulumi.FileArchive(f"../dist/{output_file}"),
        ],
    )

    return LambdaZip(
        zip_path=pulumi.FileArchive("../dist/lambda.zip"), zip_sha256=sha256_zip.stdout
    )


def create_lambda_layer(resource_prefix: str) -> aws.lambda_.LayerVersion:
    """
    Check changes on application folder to update the ZIP file for the lambda deployment
    """
    dist_folder = "lambda-layer/python"
    output_file = "lambda-layer.zip"
    poetry_install = local.Command(
        f"{resource_prefix}-poetry-install",
        dir="../app/",
        create="poetry install --only main --sync",
        update="poetry install --only main --sync",
        environment={"PYTHONUNBUFFERED": "1"},
        triggers=[
            pulumi.FileAsset("../app/poetry.lock"),
        ],
    )

    copy_libs = local.Command(
        f"{resource_prefix}-copy-python-libraries",
        dir="../",
        create=f"""mkdir -p ./dist/{dist_folder}; \
        cp -r -p ./app/.venv/lib ./dist/{dist_folder}/""",
        update=f"""mkdir -p ./dist/{dist_folder}; \
        cp -r -p ./app/.venv/lib ./dist/{dist_folder}/""",
        triggers=[
            pulumi.FileAsset("../app/poetry.lock"),
            poetry_install.stderr,
            poetry_install.stdout,
        ],
        opts=pulumi.ResourceOptions(depends_on=[poetry_install]),
    )

    cleanup = local.Command(
        f"{resource_prefix}-cleanup-layer",
        dir=f"../dist/{dist_folder}",
        create=(
            "rm -rf .venv .mypy_cache .ruff_cache .env Makefile poetry.lock pyproject.toml standalone.py"
        ),
        update="rm -rf .venv .mypy_cache .ruff_cache .env Makefile poetry.lock pyproject.toml standalone.py",
        triggers=[
            pulumi.FileAsset("../app/poetry.lock"),
            copy_libs.stdout,
            copy_libs.stderr,
            poetry_install.stderr,
            poetry_install.stdout,
        ],
        opts=pulumi.ResourceOptions(depends_on=[copy_libs]),
    )

    create_layer_zip = local.Command(
        f"{resource_prefix}-create-zip-layer",
        dir=f"../dist/{dist_folder}",
        create=f"zip -q -r -D -X -9 -A ../../{output_file} ../",
        update=f"zip -q -r -D -X -9 -A ../../{output_file} ../",
        triggers=[
            pulumi.FileAsset("../app/poetry.lock"),
            cleanup.stderr,
            cleanup.stdout,
            copy_libs.stdout,
            copy_libs.stderr,
            poetry_install.stderr,
            poetry_install.stdout,
        ],
        opts=pulumi.ResourceOptions(depends_on=[cleanup]),
    )

    sha256_zip = local.Command(
        f"{resource_prefix}-sha256-lambda-zip-layer",
        dir="../dist/",
        create=f"sha256sum {output_file} | cut -d ' ' -f1",
        update=f"sha256sum {output_file} | cut -d ' ' -f1",
        triggers=[
            pulumi.FileAsset("../app/poetry.lock"),
            create_layer_zip.stdout,
            create_layer_zip.stderr,
            cleanup.stderr,
            cleanup.stdout,
            copy_libs.stdout,
            copy_libs.stderr,
            poetry_install.stderr,
            poetry_install.stdout,
        ],
        archive_paths=[f"../dist/{output_file}"],
        opts=pulumi.ResourceOptions(depends_on=[create_layer_zip]),
    )

    return aws.lambda_.LayerVersion(
        f"{resource_prefix}-python3.12",
        layer_name=f"{resource_prefix}",
        description=f"Lambda Python layer for {resource_prefix}",
        code=sha256_zip.archive_paths[0],
        source_code_hash=sha256_zip.stdout,
        opts=pulumi.ResourceOptions(
            depends_on=[
                poetry_install,
                copy_libs,
                cleanup,
                create_layer_zip,
                sha256_zip,
            ]
        ),
    )
