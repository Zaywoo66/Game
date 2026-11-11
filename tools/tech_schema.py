"""Validate the M9 technology YAML contract and prerequisite graph."""

from event_schema import KEY, I32_MAX, I32_MIN, U32_MAX, _effect, _integer, _key, _keys


def validate_tech_branch(document, expected_branch):
    """Return field errors and valid (key, requires) pairs from one branch file."""
    errors = []
    entries = []
    if not isinstance(document, dict):
        return ["$: expected a technology branch mapping"], entries
    _keys(document, {"schema_version", "branch", "technologies"}, set(), "$", errors)
    if document.get("schema_version") != 1 or type(document.get("schema_version")) is not int:
        errors.append("$.schema_version: expected integer 1")
    branch = document.get("branch")
    _key(branch, "$.branch", errors)
    if branch != expected_branch:
        errors.append(f"$.branch: expected filename stem {expected_branch}")

    nodes = document.get("technologies")
    if not isinstance(nodes, list):
        errors.append("$.technologies: expected a list")
        return errors, entries
    for index, node in enumerate(nodes):
        path = f"$.technologies[{index}]"
        if not isinstance(node, dict):
            errors.append(f"{path}: expected a technology mapping")
            continue
        _keys(node, {"key", "cost", "requires", "pos", "effects"}, set(), path, errors)
        key = node.get("key")
        _key(key, f"{path}.key", errors)
        _integer(node.get("cost"), 1, U32_MAX, f"{path}.cost", errors)
        requirements = node.get("requires")
        if not isinstance(requirements, list):
            errors.append(f"{path}.requires: expected a list")
        else:
            seen = set()
            for requirement_index, required in enumerate(requirements):
                requirement_path = f"{path}.requires[{requirement_index}]"
                _key(required, requirement_path, errors)
                if isinstance(required, str):
                    if required in seen:
                        errors.append(f"{requirement_path}: duplicate prerequisite {required}")
                    seen.add(required)

        position = node.get("pos")
        if not isinstance(position, dict):
            errors.append(f"{path}.pos: expected a position mapping")
        else:
            _keys(position, {"x", "y"}, set(), f"{path}.pos", errors)
            for axis in ("x", "y"):
                _integer(position.get(axis), I32_MIN, I32_MAX,
                         f"{path}.pos.{axis}", errors)

        effects = node.get("effects")
        if not isinstance(effects, list):
            errors.append(f"{path}.effects: expected a list")
        else:
            for effect_index, effect in enumerate(effects):
                _effect(effect, f"{path}.effects[{effect_index}]", errors)

        if isinstance(key, str) and KEY.fullmatch(key) and isinstance(requirements, list):
            entries.append((key, tuple(required for required in requirements
                                       if isinstance(required, str) and KEY.fullmatch(required))))
    return errors, entries


def validate_prerequisites(technologies):
    """Return stable errors for missing prerequisite keys and directed cycles.

    ``technologies`` maps a technology key to its prerequisite key sequence.
    Input file parsing and per-node schema validation are handled separately.
    """
    errors = []
    for key in sorted(technologies):
        for required in technologies[key]:
            if required not in technologies:
                errors.append(f"{key}: missing prerequisite {required}")

    state = {}
    stack = []
    cycles = set()

    def visit(key):
        status = state.get(key, 0)
        if status == 2:
            return
        if status == 1:
            start = stack.index(key)
            cycle = stack[start:] + [key]
            rotations = [tuple(cycle[index:-1] + cycle[:index] + [cycle[index]])
                         for index in range(len(cycle) - 1)]
            cycles.add(min(rotations))
            return

        state[key] = 1
        stack.append(key)
        for required in sorted(set(technologies[key])):
            if required in technologies:
                visit(required)
        stack.pop()
        state[key] = 2

    for key in sorted(technologies):
        visit(key)

    errors.extend("cycle: " + " -> ".join(cycle) for cycle in sorted(cycles))
    return errors
