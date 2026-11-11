"""Validate the M7 event file contract before content enters the simulation."""

import re


KEY = re.compile(r"[a-z][a-z0-9_]*\Z", re.ASCII)
U32_MAX = (1 << 32) - 1
U64_MAX = (1 << 64) - 1
I32_MIN = -(1 << 31)
I32_MAX = (1 << 31) - 1
I64_MIN = -(1 << 63)
I64_MAX = (1 << 63) - 1


def validate_event(document):
    """Return field-specific errors for one data/events YAML document."""
    errors = []
    if not isinstance(document, dict):
        return ["$: expected one event mapping"]
    _keys(document, {"key", "title", "desc", "trigger", "options"},
          {"mean_time_to_happen", "repeatable", "cooldown_days"}, "$", errors)
    for field in ("key", "title", "desc"):
        _key(document.get(field), f"$.{field}", errors)
    _trigger(document.get("trigger"), "$.trigger", errors)
    if "mean_time_to_happen" in document:
        _integer(document["mean_time_to_happen"], 1, U64_MAX,
                 "$.mean_time_to_happen", errors)
    repeatable = document.get("repeatable", False)
    if type(repeatable) is not bool:
        errors.append("$.repeatable: expected a boolean")
    elif repeatable:
        if "cooldown_days" not in document:
            errors.append("$.cooldown_days: required when repeatable is true")
    elif "cooldown_days" in document:
        errors.append("$.cooldown_days: only valid when repeatable is true")
    if "cooldown_days" in document:
        _integer(document["cooldown_days"], 1, U64_MAX, "$.cooldown_days", errors)

    options = document.get("options")
    if not isinstance(options, list) or not options:
        errors.append("$.options: expected a nonempty list")
        return errors
    base_total = 0
    for index, option in enumerate(options):
        path = f"$.options[{index}]"
        if not isinstance(option, dict):
            errors.append(f"{path}: expected an option mapping")
            continue
        _keys(option, {"text", "effects", "ai_weight"}, {"ai_weight_if"}, path, errors)
        _key(option.get("text"), f"{path}.text", errors)
        weight = option.get("ai_weight")
        if _integer(weight, 0, U32_MAX, f"{path}.ai_weight", errors):
            base_total += weight
        effects = option.get("effects")
        if not isinstance(effects, list):
            errors.append(f"{path}.effects: expected a list")
        else:
            for effect_index, effect in enumerate(effects):
                _effect(effect, f"{path}.effects[{effect_index}]", errors)
        conditional = option.get("ai_weight_if")
        if "ai_weight_if" in option:
            condition_path = f"{path}.ai_weight_if"
            if isinstance(conditional, dict):
                _keys(conditional, {"when", "add"}, set(), condition_path, errors)
                _trigger(conditional.get("when"), f"{condition_path}.when", errors)
                _integer(conditional.get("add"), I32_MIN, I32_MAX,
                         f"{condition_path}.add", errors)
            else:
                errors.append(f"{condition_path}: expected a mapping")
    if base_total == 0:
        errors.append("$.options: at least one base ai_weight must be positive")
    return errors


def _keys(node, required, optional, path, errors):
    for field in sorted(required - node.keys()):
        errors.append(f"{path}.{field}: required field")
    for field in node:
        if field not in required | optional:
            errors.append(f"{path}.{field}: unknown field")


def _key(value, path, errors):
    if not isinstance(value, str) or KEY.fullmatch(value) is None:
        errors.append(f"{path}: expected an ASCII snake_case localization/content key")


def _integer(value, minimum, maximum, path, errors):
    if type(value) is not int or not minimum <= value <= maximum:
        errors.append(f"{path}: expected an integer in [{minimum}, {maximum}]")
        return False
    return True


def _trigger(node, path, errors):
    if not isinstance(node, dict) or len(node) != 1:
        errors.append(f"{path}: expected a single-key trigger mapping")
        return
    kind, value = next(iter(node.items()))
    if kind in ("all", "any"):
        if not isinstance(value, list):
            errors.append(f"{path}.{kind}: expected a list")
        else:
            for index, child in enumerate(value):
                _trigger(child, f"{path}.{kind}[{index}]", errors)
    elif kind == "not":
        _trigger(value, f"{path}.not", errors)
    elif kind == "country_has_tech":
        _key(value, f"{path}.country_has_tech", errors)
    elif kind == "at_war":
        if type(value) is not bool:
            errors.append(f"{path}.at_war: expected a boolean")
    else:
        errors.append(f"{path}.{kind}: unknown trigger")


def _effect(node, path, errors):
    if not isinstance(node, dict) or len(node) != 1:
        errors.append(f"{path}: expected a single-key effect mapping")
        return
    kind, value = next(iter(node.items()))
    if kind != "add_modifier":
        errors.append(f"{path}.{kind}: unknown effect")
        return
    if not isinstance(value, dict):
        errors.append(f"{path}.add_modifier: expected a modifier mapping")
        return
    modifier_path = f"{path}.add_modifier"
    _keys(value, {"key", "value", "kind"}, {"duration_days"}, modifier_path, errors)
    _key(value.get("key"), f"{modifier_path}.key", errors)
    raw = value.get("value")
    if _integer(raw, I64_MIN, I64_MAX, f"{modifier_path}.value", errors):
        if value.get("kind") == "multiplicative" and raw > I64_MAX - 1000:
            errors.append(f"{modifier_path}.value: multiplier exceeds Fixed range")
    if value.get("kind") not in ("additive", "multiplicative"):
        errors.append(f"{modifier_path}.kind: expected additive or multiplicative")
    if "duration_days" in value:
        _integer(value["duration_days"], 1, U64_MAX,
                 f"{modifier_path}.duration_days", errors)
