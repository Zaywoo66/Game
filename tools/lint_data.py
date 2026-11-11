"""Syntax check for foundation data files; schema checks arrive with content."""

import argparse
import json
from pathlib import Path
import sys


parser = argparse.ArgumentParser(description=__doc__)
parser.add_argument(
    "root", type=Path, nargs="?", default=Path(__file__).resolve().parents[1] / "data"
)
root = parser.parse_args().root
if not root.is_dir():
    parser.error(f"data directory does not exist: {root}")
errors = []
checked = 0
for path in sorted(root.rglob("*.json")):
    checked += 1
    try:
        json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeError, json.JSONDecodeError) as error:
        errors.append(f"{path.relative_to(root)}: {error}")

for error in errors:
    print(error, file=sys.stderr)
print(f"Checked {checked} JSON data files")
raise SystemExit(1 if errors else 0)

