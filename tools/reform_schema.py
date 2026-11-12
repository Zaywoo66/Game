"""Version 1 government-reform catalog checks."""

from event_schema import KEY, I64_MAX, I64_MIN, _integer, _key, _keys


def validate_reforms(document):
    errors = []
    if not isinstance(document, dict):
        return ["$: expected a reform catalog mapping"], set(), set()
    _keys(document, {"schema_version", "governments"}, set(), "$", errors)
    if type(document.get("schema_version")) is not int or document["schema_version"] != 1:
        errors.append("$.schema_version: expected integer 1")
    governments = document.get("governments")
    if not isinstance(governments, dict) or not governments:
        errors.append("$.governments: expected a nonempty mapping")
        return errors, set(), set()
    reform_keys, countries = set(), set()
    for government, tiers in governments.items():
        _key(government, f"$.governments.{government}", errors)
        countries.add(government)
        if not isinstance(tiers, list) or not tiers:
            errors.append(f"$.governments.{government}: expected nonempty tier list")
            continue
        for tier_index, tier in enumerate(tiers):
            tier_path = f"$.governments.{government}[{tier_index}]"
            if not isinstance(tier, dict):
                errors.append(f"{tier_path}: expected a mapping")
                continue
            _keys(tier, {"reforms"}, set(), tier_path, errors)
            reforms = tier.get("reforms")
            if not isinstance(reforms, list) or not reforms:
                errors.append(f"{tier_path}.reforms: expected a nonempty list")
                continue
            for index, reform in enumerate(reforms):
                path = f"{tier_path}.reforms[{index}]"
                if not isinstance(reform, dict):
                    errors.append(f"{path}: expected a mapping")
                    continue
                _keys(reform, {"key", "ai_weight", "effects"}, set(), path, errors)
                _key(reform.get("key"), f"{path}.key", errors)
                key = reform.get("key")
                if isinstance(key, str) and KEY.fullmatch(key):
                    if key in reform_keys:
                        errors.append(f"{path}.key: duplicate reform key {key}")
                    reform_keys.add(key)
                weight = reform.get("ai_weight")
                if type(weight) is not int or not 0 <= weight <= 1000:
                    errors.append(f"{path}.ai_weight: expected integer 0..1000")
                effects = reform.get("effects")
                if not isinstance(effects, list) or not effects:
                    errors.append(f"{path}.effects: expected a nonempty list")
                    continue
                for effect_index, effect in enumerate(effects):
                    effect_path = f"{path}.effects[{effect_index}]"
                    if not isinstance(effect, dict):
                        errors.append(f"{effect_path}: expected a mapping")
                        continue
                    _keys(effect, {"key", "value", "kind"}, set(), effect_path, errors)
                    _key(effect.get("key"), f"{effect_path}.key", errors)
                    value = effect.get("value")
                    if _integer(value, I64_MIN, I64_MAX, f"{effect_path}.value", errors):
                        if effect.get("kind") == "multiplicative" and value > I64_MAX - 1000:
                            errors.append(f"{effect_path}.value: multiplier exceeds Fixed range")
                    if effect.get("kind") not in ("additive", "multiplicative"):
                        errors.append(f"{effect_path}.kind: expected additive or multiplicative")
    return errors, reform_keys, countries
