"""`test_models.py` checks that the models accept and reject requests as the spec says.

Partitionings of a `GameServerCreate` request, and the tests that cover each part:

| Partitioning | Part | Tests |
| --- | --- | --- |
| `max_player_count` | below 1 | `test_rejects_zero_max_player_count` |
| | at least 1 | `test_defaults_config`, `test_accepts_config` |
| `config` | omitted | `test_defaults_config` |
| | given | `test_accepts_config` |
| other fields | none | `test_defaults_config`, `test_accepts_config` |
| | unknown | `test_rejects_unknown_field` |
"""

import pydantic
import pytest

from management_server import models


def test_defaults_config() -> None:
    """`test_defaults_config` checks that an omitted `config` becomes an empty one."""
    request: models.GameServerCreate = models.GameServerCreate.model_validate(
        {"max_player_count": 1}
    )
    assert request.config == models.GameServerConfig()


def test_accepts_config() -> None:
    """`test_accepts_config` checks that a given `config` is kept."""
    request: models.GameServerCreate = models.GameServerCreate.model_validate(
        {"config": {}, "max_player_count": 4}
    )
    assert request.max_player_count == 4


def test_rejects_zero_max_player_count() -> None:
    """`test_rejects_zero_max_player_count` checks that a game server needs room for a player."""
    with pytest.raises(pydantic.ValidationError):
        models.GameServerCreate.model_validate({"max_player_count": 0})


def test_rejects_unknown_field() -> None:
    """`test_rejects_unknown_field` checks that a field the spec doesn't define is an error."""
    with pytest.raises(pydantic.ValidationError):
        models.GameServerCreate.model_validate({"max_player_count": 1, "map": "cube"})
