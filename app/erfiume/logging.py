"""
Module to start and configure logging.
"""

from aws_lambda_powertools import Logger

logger = Logger(service="erfiume")
