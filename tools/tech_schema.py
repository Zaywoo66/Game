"""Validation helpers for the M9 technology prerequisite graph."""


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
