"""`errors.py` defines the errors that the internal management API and launchers raise.

Interface:
    NotFoundError: No game server or player has the given ID.

Example:
    >>> raise NotFoundError(f"no game server {game_server_id}")
"""


class NotFoundError(Exception):
    """`NotFoundError` means that no game server or player has the given ID.

    Example:
        >>> raise NotFoundError(f"no game server {game_server_id}")
    """
