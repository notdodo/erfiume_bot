"""Stack-level Pulumi configuration helpers."""

from __future__ import annotations

from dataclasses import dataclass

from pulumi import Config, Output, get_stack


@dataclass(frozen=True)
class StackConfig:
    """Immutable configuration shared across infrastructure modules."""

    resources_prefix: str
    sync_minutes_rate_medium: int
    sync_minutes_rate_emergency: int
    emergency: bool
    custom_domain_name: str
    certificate_arn: str
    cloudflare_zone_id: str
    stations_scan_page_size: str

    @classmethod
    def load(cls) -> StackConfig:
        """Build stack configuration from project defaults and Pulumi secrets."""
        return cls(
            resources_prefix="erfiume",
            sync_minutes_rate_medium=2 * 60,
            sync_minutes_rate_emergency=20,
            emergency=False,
            custom_domain_name="erfiume.thedodo.xyz",
            certificate_arn="arn:aws:acm:eu-west-1:841162699174:certificate/109ca827-8d70-4e11-8995-0b3dbdbd0510",
            cloudflare_zone_id="cec5bf01afed114303a536c264a1f394",
            stations_scan_page_size="25",
        )

    @property
    def environment(self) -> str:
        """Return the active Pulumi stack name."""
        return get_stack()

    @property
    def fetcher_rate_minutes(self) -> int:
        """Return the scheduler cadence for the fetcher Lambda."""
        return (
            self.sync_minutes_rate_emergency
            if self.emergency
            else self.sync_minutes_rate_medium
        )

    def require_secret(self, key: str) -> Output[str]:
        """Load a required Pulumi secret for the current stack."""
        return Config().require_secret(key)
