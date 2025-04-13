"""Pulumi resources to create and configure Lambda functions"""

from __future__ import annotations

from enum import Enum
from typing import TYPE_CHECKING

import pulumi
from pulumi_aws import cloudwatch, lambda_

from .helpers import format_resource_name

if TYPE_CHECKING:
    from .iam import LambdaRole


class FunctionRuntime(Enum):
    """The environment of the function executing the code"""

    RUST = lambda_.Runtime.CUSTOM_AL2023


class Function(pulumi.ComponentResource):
    """
    A Pulumi custom resource to create a Lambda function.

    :param name str: The name of the function to create.
    :param memory int: The memory of the function to allocate (MB).
    :param timeout Optional[int]: The timeout in seconds for the function execution (default 3).
    :param code_runtime Optional[FunctionRuntime]: The type of the environment to execute the function (default Rust).
    :param role Optional[LambdaRole]: The execution IAM role to use in the function.
    :param variables Optional[dict[str,str]]: The environmental variables to use inside the function.
    :param opts pulumi.ResourceOptions | None: Pulumi resource options for the custom resource.
    """

    def __init__(
        self,
        name: str,
        memory: int,
        timeout: int | None = 3,
        code_runtime: FunctionRuntime | None = FunctionRuntime.RUST,
        role: LambdaRole | None = None,
        variables: dict[str, str | pulumi.Output[str]] | None = None,
        opts: pulumi.ResourceOptions | None = None,
    ) -> None:
        """
        Initialize the Function class.
        """
        self.name = name
        self.resource_name = f"{format_resource_name(name, self)}-function"
        super().__init__("notdodo:erfiume:Function", self.name, {}, opts)

        self.function = lambda_.Function(
            self.resource_name,
            code=pulumi.FileArchive("./er_fiume/dummy.zip"),
            name=self.name,
            role=role.arn if role else None,
            handler="bootstrap",
            runtime=code_runtime.value if code_runtime else None,
            environment={
                "variables": variables,
            },
            memory_size=memory,
            timeout=timeout,
            opts=pulumi.ResourceOptions.merge(
                pulumi.ResourceOptions(parent=self), opts
            ),
        )

        cloudwatch.LogGroup(
            self.resource_name,
            log_group_class="STANDARD",
            name=f"/aws/lambda/{self.name}",
            retention_in_days=14,
            opts=pulumi.ResourceOptions.merge(
                pulumi.ResourceOptions(parent=self), opts
            ),
        )

        self.arn = self.function.arn
        self.register_outputs({"function": self.function})
