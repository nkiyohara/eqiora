#!/usr/bin/env python3
"""Run the installed-wheel private gallery-media evidence case."""

from __future__ import annotations

import sys


def main() -> int:
    """Refuse success until the frozen gallery-media contract is implemented."""

    print("private gallery-media evidence is not implemented", file=sys.stderr)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
