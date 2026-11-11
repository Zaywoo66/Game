"""Check JSON/YAML syntax, raw Fixed values, and content contracts."""

import argparse
import json
from pathlib import Path
import sys

import yaml

from event_schema import KEY, noncanonical_scalar_tokens, validate_event
from tech_schema import validate_prerequisites, validate_tech_branch


RAW_FIXED = (-(1 << 63), (1 << 63) - 1, "expected a raw Fixed integer in the signed 64-bit range")
DURATION_DAYS = (1, (1 << 64) - 1, "expected a positive integer day count in the unsigned 64-bit range")
DATA_SUFFIXES = {".json", ".yaml", ".yml"}
EVENT_LOCALE = Path("locales/events.en.json")


def unique_locale_pairs(pairs):
    """Reject duplicate event translation keys before JSON discards them."""
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate event localization key {key}")
        result[key] = value
    return result


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
    event_references = {}
    event_locale = None
    tech_branches = {}
    tech_keys = {}
    tech_graph = {}
    for path in sorted(root.rglob("*")):
        suffix = path.suffix.lower()
        if suffix not in DATA_SUFFIXES or not path.is_file():
            continue

        checked += 1
        relative_path = path.relative_to(root)
        try:
            text = path.read_text(encoding="utf-8")
            if suffix == ".json":
                hook = unique_locale_pairs if relative_path == EVENT_LOCALE else None
                document = json.loads(text, object_pairs_hook=hook)
            else:
                document = yaml.safe_load(text)
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
            errors.extend(
                f"{relative_path}: {error}" for error in noncanonical_scalar_tokens(text)
            )
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
            if not event_errors:
                for key in (document["title"], document["desc"],
                            *(option["text"] for option in document["options"])):
                    event_references.setdefault(key, relative_path)

        if relative_path == EVENT_LOCALE:
            event_locale = document
            if not isinstance(document, dict):
                errors.append(f"{relative_path}: expected a flat localization mapping")
                continue
            for key, value in document.items():
                if not isinstance(key, str) or KEY.fullmatch(key) is None:
                    errors.append(f"{relative_path}: invalid localization key {key!r}")
                if not isinstance(value, str) or not value.strip():
                    errors.append(f"{relative_path}: {key}: expected nonempty English text")

        if relative_path.parts[0] == "techs":
            if suffix not in {".yaml", ".yml"}:
                errors.append(f"{relative_path}: technologies must use YAML")
                continue
            errors.extend(
                f"{relative_path}: {error}" for error in noncanonical_scalar_tokens(text)
            )
            branch_errors, entries = validate_tech_branch(document, path.stem)
            errors.extend(f"{relative_path}: {error}" for error in branch_errors)
            if not branch_errors:
                if document["branch"] in tech_branches:
                    errors.append(
                        f"{relative_path}: duplicate technology branch {document['branch']} "
                        f"also in {tech_branches[document['branch']]}"
                    )
                else:
                    tech_branches[document["branch"]] = relative_path
                for key, requirements in entries:
                    if key in tech_keys:
                        errors.append(
                            f"{relative_path}: duplicate technology key {key} also in {tech_keys[key]}"
                        )
                    else:
                        tech_keys[key] = relative_path
                        tech_graph[key] = requirements

    errors.extend(f"techs: {error}" for error in validate_prerequisites(tech_graph))
    if event_references and event_locale is None:
        errors.append(f"{EVENT_LOCALE}: required for event localization keys")
    elif isinstance(event_locale, dict):
        for key, source in sorted(event_references.items()):
            if key not in event_locale:
                errors.append(f"{source}: missing English event localization key {key}")

    for error in errors:
        print(error, file=sys.stderr)
    print(f"Checked {checked} JSON and YAML data files")
    return 1 if errors else 0


if __name__ == "__main__":
    raise SystemExit(main())
