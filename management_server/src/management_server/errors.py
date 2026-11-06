"""`errors.py` defines the errors that the internal management API and launchers raise.

Interface:
    NotFoundError: No game server or player has the given ID.
    InvalidPageTokenError: A page token isn't one the management server returned.

Example:
    >>> raise NotFoundError(f"no game server {game_server_id}")
"""


class NotFoundError(Exception):
    """`NotFoundError` means that no game server or player has the given ID.

    Example:
        >>> raise NotFoundError(f"no game server {game_server_id}")
    """


class InvalidPageTokenError(Exception):
    """`InvalidPageTokenError` means that a page token isn't one the management server returned.

    Example:
        >>> raise InvalidPageTokenError(f"invalid page token {page_token!r}")
    """
