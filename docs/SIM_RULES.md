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
   - On the first day of each month, each operational bilateral trade route
     adds 1 whole treasury unit to each participant after tax and before
     spending. The path is recalculated from current capitals, owned/allied
     land and sea connections; war or a blocked required port pauses income.
     Trade income does not count as taxable income for vassal tribute.
   - At the start of a new Gregorian month, after the economy step, each country
     gains 10 whole research points up to a stockpile of 1000. The host-side AI
     chooses at the start of the month before this grant, so the new points are
     spendable on its next monthly choice.
   - A port under an unopposed wartime fleet blockade contributes no tax on a monthly tick; the budget forecast uses the same current fleet positions.
6. Diplomacy timers
   - On the first day of each month, each country not participating in an
     active war loses 1 aggressive-expansion point, down to 0. Countries in a
     war do not lose points that month.
7. Armies move
8. Combat/sieges
9. Navy
10. Events/triggers evaluation
11. Emit deltas

Newly raised armies spend 30 daily ticks recruiting at their capital. They
cannot move, fight, siege, merge, split, retreat, or embark during recruitment.
The army becomes operational on the following tick after the countdown reaches
zero. The UI marks recruiting armies and exposes their remaining days.

## AI support
- AI query helpers are read-only. AI uses an RNG derived from `(seed, tick, country_id)`.
- Each new simulation mechanic includes the query helpers and legal-command checks AI needs.

## Testing
- Unit tests per mechanic; property/replay tests for determinism; golden save-file test (load -> N ticks -> compare hash).
- Bench with `criterion` for the tick on the full map.
