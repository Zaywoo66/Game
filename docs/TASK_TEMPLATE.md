# TASK_TEMPLATE

```
Goal:        one sentence
Milestone:   Mxx
Module:      sim | net | data | app | ui | map | tools
Inputs/Outputs: types, file formats, API signatures
Acceptance:  checklist incl. named tests
Do not touch: files/modules that must not change
Docs to read: max 2 files
```

## Good vs bad
- Bad: "implement the war system".
- Good: "Implement `Army::advance(tick)` along a stored route over the province graph; tests: two hostile armies entering one province trigger a Battle event; movement is deterministic."

## Sizing
A task should fit one PR (< ~500 changed lines excluding data). If bigger, split it.

## Review checklist
Tests present and meaningful? Determinism rules respected? No hardcoded content? Docs updated? Unrelated changes absent?
