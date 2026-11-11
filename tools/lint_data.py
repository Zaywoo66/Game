"""Check JSON/YAML syntax, raw Fixed values, and M7 event contracts."""

import argparse
import json
from pathlib import Path
import sys

import yaml

from event_schema import validate_event


RAW_FIXED = (-(1 << 63), (1 << 63) - 1, "expected a raw Fixed integer in the signed 64-bit range")
DURATION_DAYS = (1, (1 << 64) - 1, "expected a positive integer day count in the unsigned 64-bit range")
DATA_SUFFIXES = {".json", ".yaml", ".yml"}


def modifier_fields(document):
    """Yield the paths, values, and integer constraints of modifier fields."""
    pending = [("$", document)]
    visited = set()
    while pending:
        location, node = pending.pop()
        if not isinstance(node, (dict, list)) or id(node) in visited:
            continue
        visited.add(id(node))

        if isinstance(node, dict):
            effect = node.get("add_modifier")
            if isinstance(effect, dict):
                if "value" in effect:
                    yield f"{location}.add_modifier.value", effect["value"], RAW_FIXED
                if "duration_days" in effect:
                    yield f"{location}.add_modifier.duration_days", effect["duration_days"], DURATION_DAYS

            modifiers = node.get("modifiers")
            if isinstance(modifiers, list):
                for index, modifier in enumerate(modifiers):
                    if isinstance(modifier, dict) and "value" in modifier:
                        yield f"{location}.modifiers[{index}].value", modifier["value"], RAW_FIXED

            pending.extend(
                (f"{location}.{key}", child)
                for key, child in reversed(node.items())
                if isinstance(child, (dict, list))
            )
        else:
            pending.extend(
                (f"{location}[{index}]", node[index])
                for index in range(len(node) - 1, -1, -1)
                if isinstance(node[index], (dict, list))
            )


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "root", type=Path, nargs="?", default=Path(__file__).resolve().parents[1] / "data"
    )
    root = parser.parse_args().root
    if not root.is_dir():
        parser.error(f"data directory does not exist: {root}")

    errors = []
    checked = 0
    event_keys = {}
    for path in sorted(root.rglob("*")):
        suffix = path.suffix.lower()
        if suffix not in DATA_SUFFIXES or not path.is_file():
            continue

        checked += 1
        relative_path = path.relative_to(root)
        try:
            text = path.read_text(encoding="utf-8")
            document = json.loads(text) if suffix == ".json" else yaml.safe_load(text)
        except (OSError, ValueError, RecursionError, yaml.YAMLError) as error:
            errors.append(f"{relative_path}: {error}")
            continue

        for field, value, (minimum, maximum, message) in modifier_fields(document):
            # An exact type check excludes bool, which is a subclass of int.
            if type(value) is not int or not minimum <= value <= maximum:
                errors.append(f"{relative_path}: {field}: {message}")

        if relative_path.parts[0] == "events":
            if suffix not in {".yaml", ".yml"}:
                errors.append(f"{relative_path}: events must use YAML")
                continue
            try:
                event_errors = validate_event(document)
            except RecursionError:
                event_errors = ["$: event trigger nesting is too deep or cyclic"]
            errors.extend(f"{relative_path}: {error}" for error in event_errors)
            if not event_errors and document["key"] in event_keys:
                errors.append(
                    f"{relative_path}: $.key: duplicate event key also in "
                    f"{event_keys[document['key']]}"
                )
            elif not event_errors:
                event_keys[document["key"]] = relative_path

    for error in errors:
        print(error, file=sys.stderr)
    print(f"Checked {checked} JSON and YAML data files")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
