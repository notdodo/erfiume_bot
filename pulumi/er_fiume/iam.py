"""Pulumi resources to create and configure IAM Roles"""

from __future__ import annotations

import pulumi
from pulumi_aws import get_caller_identity, iam

from .helpers import format_resource_name


class LambdaRole(pulumi.ComponentResource):
    """
    A Pulumi custom resource to create a IAM LambdaRole.

    :param name str: The name of the role to create.
    :param permissions list[dict[str, str | list[str | pulumi.Output[str]]]]: The inline policy to create.
    :param path str: The path for the role.
    :param opts pulumi.ResourceOptions | None: Pulumi resource options for the custom resource.
    """

    def __init__(
        self,
        name: str,
        permissions: list[dict[str, str | list[str | pulumi.Output[str]]]],
        path: str | None = None,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """
        Initialize the IAM class.
        """
        self.name = name
        self.resource_name = f"{format_resource_name(name, self)}-role"
        super().__init__("notdodo:erfiume:LambdaRole", self.name, {}, opts)
        path = path or "/"

        self.role = iam.Role(
            self.resource_name,
            name=self.name,
            path=path,
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
                        "Condition": [
                            {
                                "Test": "StringLike",
                                "Variable": "aws:SourceArn",
                                "Values": [
                                    f"arn:aws:lambda:*:{get_caller_identity().account_id}:function:{path.strip('/') or ''}*"
                                ],
                            }
                        ],
                    }
                ]
            ).json,
            managed_policy_arns=[
                "arn:aws:iam::aws:policy/service-role/AWSLambdaBasicExecutionRole"
            ],
            inline_policies=[
                iam.RoleInlinePolicyArgs(
                    name=f"{self.resource_name}-inline-policy",
                    policy=iam.get_policy_document_output(
                        statements=permissions,
                    ).json,
                )
            ],
            opts=pulumi.ResourceOptions.merge(
                pulumi.ResourceOptions(parent=self), opts
            ),
        )
        self.arn = self.role.arn
        self.register_outputs({"lambdarole": self.role})


class GenericRole(pulumi.ComponentResource):
    """
    A Pulumi custom resource to create a IAM GenericRole.

    :param name str: The name of the role to create.
    :param for_services str: The service to use as trust policy.
    :param permissions list[dict[str, str | list[str | pulumi.Output[str]]]]: The inline policy to create.
    :param path str: The path for the role.
    :param opts pulumi.ResourceOptions | None: Pulumi resource options for the custom resource.
    """

    def __init__(
        self,
        name: str,
        for_services: list[str],
        permissions: list[dict[str, str | list[str | pulumi.Output[str]]]],
        path: str | None = None,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """
        Initialize the IAM class.
        """
        self.name = name
        self.resource_name = f"{format_resource_name(name, self)}-role"
        super().__init__("notdodo:erfiume:IAMRole", self.name, {}, opts)
        path = path or "/"

        self.role = iam.Role(
            self.resource_name,
            name=self.name,
            path=path,
            assume_role_policy=iam.get_policy_document(
                statements=[
                    {
                        "Effect": "Allow",
                        "Principals": [
                            {
                                "Type": "Service",
                                "Identifiers": for_services,
                            }
                        ],
                        "Actions": ["sts:AssumeRole"],
                        "Condition": [
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
                    name=f"{self.resource_name}-inline-policy",
                    policy=iam.get_policy_document_output(
                        statements=permissions,
                    ).json,
                )
            ],
            opts=pulumi.ResourceOptions.merge(
                pulumi.ResourceOptions(parent=self), opts
            ),
        )
        self.arn = self.role.arn
        self.register_outputs({"iamrole": self.role})
