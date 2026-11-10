"""`clock.py` keeps the prototype's time, which runs like real time but can be skipped forward.

Skipping lets a reviewer wait out a cooldown of hours or a season of months in one click.

Interface:
    SimClock: The clock.

Example:
    >>> clock = SimClock(); clock.skip(3600.0); clock.readSeconds() >= 3600.0
    True
"""

import time
from collections.abc import Callable


class SimClock:
    """`SimClock` counts seconds since the prototype's world began.

    Args:
        read_real_s: A function that returns real time in seconds (default `time.monotonic`).
        start_s: The time the clock starts at (default 0).

    Example:
        >>> clock = SimClock(read_real_s=lambda: 5.0); clock.readSeconds()
        0.0
    """

    def __init__(
        self, read_real_s: Callable[[], float] = time.monotonic, start_s: float = 0.0
    ) -> None:
        self._read_real_s: Callable[[], float] = read_real_s
        self._real_start_s: float = read_real_s()
        self._skipped_s: float = start_s

    def readSeconds(self) -> float:
        """`readSeconds` returns the seconds since the world began, skips included.

        Returns:
            The prototype's current time.

        Example:
            >>> SimClock(read_real_s=lambda: 1.0).readSeconds()
            0.0
        """
        return self._read_real_s() - self._real_start_s + self._skipped_s

    def skip(self, seconds: float) -> None:
        """`skip` moves the clock forward by `seconds`.

        Args:
            seconds: How far to skip, more than 0.

        Raises:
            ValueError: `seconds` isn't positive.

        Example:
            >>> SimClock().skip(60.0)
        """
        if seconds <= 0:
            raise ValueError(f"seconds must be positive, got {seconds}")
        self._skipped_s += seconds
