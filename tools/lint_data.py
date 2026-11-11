"""Syntax check for foundation data files; schema checks arrive with content."""

from pathlib import Path
import json
import sys


root = Path(__file__).resolve().parents[1] / "data"
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

