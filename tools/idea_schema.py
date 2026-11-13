"""Version 1 national-idea content checks."""

from event_schema import KEY, I64_MAX, I64_MIN, _integer, _key, _keys, _trigger


def validate_ideas(document, expected_country):
    errors = []
    if not isinstance(document, dict):
        return ["$: expected an idea catalog mapping"], []
    _keys(document, {"schema_version", "country", "ideas"}, set(), "$", errors)
    if type(document.get("schema_version")) is not int or document["schema_version"] != 1:
        errors.append("$.schema_version: expected integer 1")
    _key(document.get("country"), "$.country", errors)
    if document.get("country") != expected_country:
        errors.append("$.country: must match filename stem")
    ideas = document.get("ideas")
    if not isinstance(ideas, list) or not ideas:
        errors.append("$.ideas: expected a nonempty list")
        return errors, []
    keys = set()
    valid_keys = []
    for index, idea in enumerate(ideas):
        path = f"$.ideas[{index}]"
        if not isinstance(idea, dict):
            errors.append(f"{path}: expected a mapping")
            continue
        _keys(idea, {"key", "name", "modifiers", "unlock"}, set(), path, errors)
        _key(idea.get("key"), f"{path}.key", errors)
        _key(idea.get("name"), f"{path}.name", errors)
        key = idea.get("key")
        if isinstance(key, str) and KEY.fullmatch(key):
            if key in keys:
                errors.append(f"{path}.key: duplicate idea key")
            else:
                keys.add(key)
                valid_keys.append(key)
        _trigger(idea.get("unlock"), f"{path}.unlock", errors)
        modifiers = idea.get("modifiers")
        if not isinstance(modifiers, list) or not modifiers:
            errors.append(f"{path}.modifiers: expected a nonempty list")
            continue
        for modifier_index, modifier in enumerate(modifiers):
            modifier_path = f"{path}.modifiers[{modifier_index}]"
            if not isinstance(modifier, dict):
                errors.append(f"{modifier_path}: expected a mapping")
                continue
            _keys(modifier, {"key", "value", "kind"}, set(), modifier_path, errors)
            _key(modifier.get("key"), f"{modifier_path}.key", errors)
            value = modifier.get("value")
            if _integer(value, I64_MIN, I64_MAX, f"{modifier_path}.value", errors):
                if modifier.get("kind") == "multiplicative" and value > I64_MAX - 1000:
                    errors.append(f"{modifier_path}.value: multiplier exceeds Fixed range")
            if modifier.get("kind") not in ("additive", "multiplicative"):
                errors.append(f"{modifier_path}.kind: expected additive or multiplicative")
    return errors, valid_keys


def technology_references(document):
    """Yield technology keys mentioned by idea unlock triggers."""
    if not isinstance(document, dict):
        return
    for idea in document.get("ideas", []):
        if not isinstance(idea, dict):
            continue
        pending = [idea.get("unlock")]
        while pending:
            node = pending.pop()
            if not isinstance(node, dict):
                continue
            for kind, value in node.items():
                if kind == "country_has_tech" and isinstance(value, str):
                    yield value
                elif kind in ("all", "any") and isinstance(value, list):
                    pending.extend(value)
                elif kind == "not":
                    pending.append(value)
