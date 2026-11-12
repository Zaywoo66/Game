# SIM_RULES (crates/sim)

## Determinism
- Same initial state + same seed + same command sequence => bit-identical state. Test this (hash state after N ticks).
- Numbers: integers or fixed-point (e.g. `Fixed = i64` with 1/1000 scale). No floats.
- Collections: `Vec`, `BTreeMap`, `IndexMap`. Iteration order must be defined.
- RNG: one seeded PRNG in `GameState`, advanced only inside `tick()`/`apply()`. Never call RNG in UI or net code.
- No wall-clock time. Time is `Date` derived from tick count.

## Ids
- Newtype ids (`ProvinceId`, `CountryId`, `CharacterId`, `ArmyId`, ...). Stable across save/load. Data files reference ids by string key; `data` crate maps keys -> ids.

## Modifiers
- `Modifier { key: ModKey, value: Fixed, kind: Additive | Multiplicative }`.
- Modifier values in data are raw Fixed integers in thousandths for both additive and multiplicative kinds.
- Sources: tech, idea, trait, law, event, terrain, building. Each source has an id for removal/expiry.
- Effective value = (base + sum additive) * product(1 + multiplicative). Document any deviation here.

## Triggers and Effects
- Trigger: boolean tree (`all`, `any`, `not`, leaf conditions). Pure function of `GameState`.
- Effect: list of state changes (add modifier, change treasury, set owner, fire event...). Executed only inside `apply/tick`.
- Adding a new leaf trigger/effect requires: enum variant, data-schema entry in DATA_FORMATS.md, unit test.

## Commands
- Every command: validate -> apply -> emit `Delta`. Validation failure returns a typed reason (UI shows it in tooltips).

## Tick order (document changes here)
1. Date advance
2. Scheduled events/expiries
3. AI decisions. The host calls `ai::decide` for AI countries in ascending `CountryId` order, then validates and applies their commands.
4. Characters (age, death, traits)
5. Economy
   - At the start of a new Gregorian month, after the economy step, each country
     gains 10 whole research points up to a stockpile of 1000. The host-side AI
     chooses at the start of the month before this grant, so the new points are
     spendable on its next monthly choice.
   - A port under an unopposed wartime fleet blockade contributes no tax on a monthly tick; the budget forecast uses the same current fleet positions.
6. Diplomacy timers
7. Armies move
8. Combat/sieges
9. Navy
10. Events/triggers evaluation
11. Emit deltas

## AI support
- AI query helpers are read-only. AI uses an RNG derived from `(seed, tick, country_id)`.
- Each new simulation mechanic includes the query helpers and legal-command checks AI needs.

## Testing
- Unit tests per mechanic; property/replay tests for determinism; golden save-file test (load -> N ticks -> compare hash).
- Bench with `criterion` for the tick on the full map.
