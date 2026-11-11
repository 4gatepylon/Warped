"""`secret_manager.py` plays Google Cloud's Secret Manager: the backend's keys and shared secrets.

Interface:
    SECRET_NAMES: The secrets the system needs.
    SecretManager: Creates the secrets once and hands them to the components that read them.

Example:
    >>> secrets = SecretManager(bus); secrets.createMissingSecrets()
    >>> secrets.readSecret("backend", "access_token_key")
    '...'

Must know:
    Limitations:
        - The fake Google and the fake Stripe read their copy of a shared secret (the client
          secret, the webhook secret) straight from the table, because in reality each was
          registered in that company's console once, by hand.
"""

import secrets

from .bus import Bus
from .rules import NotFoundError

SECRET_NAMES: tuple[str, ...] = (
    "access_token_key",
    "sign_up_token_key",
    "google_client_secret",
    "stripe_api_key",
    "stripe_webhook_secret",
)
_SECRET_BYTES: int = 32


class SecretManager:
    """`SecretManager` keeps the `secret` table, which the system view shows without the values.

    Args:
        bus: The shared database, clock and message log.

    Example:
        >>> SecretManager(bus).readSecret("backend", "stripe_webhook_secret")
    """

    def __init__(self, bus: Bus):
        self._bus: Bus = bus

    def createMissingSecrets(self) -> None:
        """`createMissingSecrets` gives each missing secret a random value, as a deploy would.

        Example:
            >>> secrets.createMissingSecrets()
        """
        for name in SECRET_NAMES:
            self._bus.execute(
                "INSERT OR IGNORE INTO secret (name, value) VALUES (?, ?)",
                (name, secrets.token_urlsafe(_SECRET_BYTES)),
            )

    def readSecret(self, reader: str, name: str) -> str:
        """`readSecret` returns a secret's value to a component, and records that it read it.

        Args:
            reader: The component that reads it, one of `bus.COMPONENTS`.
            name: The secret, one of `SECRET_NAMES`.

        Raises:
            NotFoundError: The secret doesn't exist.

        Example:
            >>> secrets.readSecret("backend", "access_token_key")
        """
        self._bus.send(self._bus.readSeconds(), reader, "secret_manager", "access secret", name)
        rows: list[dict] = self._bus.read("SELECT value FROM secret WHERE name = ?", (name,))
        if len(rows) == 0:
            raise NotFoundError(f"There is no secret named {name}.")
        return rows[0]["value"]
