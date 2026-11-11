# DATA_FORMATS
Content files in `/data`, JSON or YAML, validated by `crates/data` on load and by CI linter. Keys are snake_case strings; cross-references by key. Keep each file small; one entity type per file or folder.

## Provinces (`data/map/<region>/provinces.json`)

Version 1 is a JSON object with `schema_version: 1`, a positive `map_version`
(integer up to 2³² − 1), `map_hash` (64 lowercase SHA-256 hex digits), and a
`provinces` array. Each province has these required fields:

| Field | Representation |
|---|---|
| `id` | Positive 32-bit unsigned integer. ID 0 is reserved. IDs are unique and may be sparse. |
| `key` | Unique ASCII snake_case string, matching `[a-z][a-z0-9]*(?:_[a-z0-9]+)*`. |
| `kind` | `land`, `sea`, or `lake`. |
| `terrain` | ASCII snake_case terrain key. |
| `centroid` | `[x, y]` in integer pixel units, each a signed 32-bit integer. Raster bounds are checked in M2. |
| `neighbors` | Array of directed `{ "id": <target ID>, "edge": "normal\|river\|strait\|port" }` records. |
| `ports` | Array of IDs for sea provinces. |
| `development` | Nonnegative 32-bit unsigned integer in whole development points. |

M4 uses `schema_version: 2` and adds required `cost` to each neighbor.
Cost is a positive unsigned 32-bit integer in whole daily movement ticks.
The corresponding reverse edge must have the same cost. A legacy v1 file
loads with a cost of one tick for every edge. Schema v3 retains weighted edges
and adds projection and hierarchy fields below.

Neighbor and port references within this file use numeric IDs. Other content
files reference provinces by their stable string key. Each directed neighbor
record must have a reverse record of the same type. Repeated records with the
same source, target, and type are invalid. Different types between the same
endpoints are valid. Port IDs must exist, be unique within the list, and point
to a sea province. Fields outside this schema
are rejected.

### Map hash and saves

For schema v1 and v2, compute `map_hash` from compact UTF-8 JSON of the
`provinces` array. For v3, hash the object `{"projection":...,"provinces":...}`
in that field order. Write projection fields `pixels_per_degree`, `origin` in
that order and append `name_key`, `area` after each province's `development`.
Emit no whitespace outside JSON strings and no
trailing newline. Write each province's fields in this exact order:
`id`, `key`, `kind`, `terrain`, `centroid`, `neighbors`, `ports`,
`development`, then for v3 `name_key`, `area`. Write v1 neighbor fields in the order `id`, `edge`,
and v2/v3 neighbor fields in the order `id`, `edge`, `cost`.
Write integers as decimal JSON integers without leading zeros, using `0` for
zero. String values in the canonical form use the ASCII field values from
the table and need no escape sequences.

Sort provinces by ID and ports by ascending ID. Sort neighbor records first
by target ID, then by type in the order `normal`, `river`, `strait`, `port`.
Encode the SHA-256 digest as lowercase hexadecimal. Source whitespace,
object field order, and array order for provinces, neighbors, and ports do
not affect the hash. The version fields are outside the digest.
The fixture `crates/data/tests/fixtures/provinces.json` has canonical hash
`ef610c0ac68bf9e1c0c84fb75f5a03a69dc761f1efad9790e41aa7e5414f347a`.

The loader recomputes and verifies the hash. A save must record both
`map_version` and `map_hash`. When loading a save in M1.7, compare both to
the loaded graph's identity and reject a mismatch before accepting game
state, even if only the version differs. Use
`graph.identity().verify_save(saved_identity)` for that check.

Run `cargo run --locked -p strategy-data --bin lint-provinces -- data/map/western_europe/provinces.json`
to validate the generated file. CI tests the validator against fixtures and
requires the generated map to pass validation.

### Map hierarchy and projection (schema version 3)

| Field or file | Representation |
|---|---|
| `projection.pixels_per_degree` | Positive unsigned 32-bit integer in pixels per degree of longitude and latitude. |
| `projection.origin` | Two signed 32-bit integer offsets in pixels from the northwest global origin (180° W, 90° N). |
| `provinces[].name_key` | ASCII snake_case localization key for the settlement or province display name. |
| `provinces[].area` | Stable area key referencing `areas.yaml`. |
| `data/map/<region>/areas.yaml` | Schema v1, reference year, assignment method and area records with `key`, `name_key`, `name`, `region`, `kind` and a source URL when historically classified. |
| `data/map/<region>/regions.yaml` | Schema v1 and region records with `key`, `name_key` and `name`. |
| `data/map/<region>/names.en.json` | Schema v1 and English fallback names keyed by localization key. |

MAP_SCALE.md defines the hierarchy and reserved ID ranges. The generated Western Europe map uses `map_version: 3`. The loader still verifies schema v1/v2 maps with their original hash rules. Older saves are accepted only when loaded against their own map identity.

## Saved games (schema version 1)

`strategy_data::save_game` encodes an authoritative simulation snapshot in
compact UTF-8 JSON without a trailing newline. The envelope fields are
`schema_version`, `map_version`, `map_hash`, and `state` in that order. The
map identity comes from the game's graph. The static graph is supplied
separately to `load_game` and is never serialized in the save. Loading first
checks the save version and compares both map identity fields against the
supplied graph before decoding its game state.

The state contains the clock, RNG, country records, and scheduled modifier
grants. Its numeric fields use these representations:

| Field | Representation and unit |
|---|---|
| `schema_version` | Unsigned 32-bit schema version number. Save v1 requires `1`. |
| `map_version` | Positive unsigned 32-bit map version number. |
| `start_date` | `[year, month, day]` in the proleptic Gregorian calendar. Year is unsigned 32-bit; month and day are unsigned 8-bit and start at one. |
| `tick`, scheduled `due`, and `expires_at` | Unsigned 64-bit ticks since the scenario started, at one tick per game day. A null expiry means a permanent source. A timed active source must expire after the saved tick. A scheduled expiry must be after its due tick. |
| `speed` | Unsigned 8-bit speed level, 1 through 5. |
| `seed`, `rng_state` | Unsigned 64-bit seed and generator state, respectively. |
| Country IDs, player IDs, and source IDs | Unsigned 32-bit identifiers. |
| Modifier `value` | Signed 64-bit integer in raw Fixed thousandths. A multiplicative value must allow `1 + value` to fit in Fixed. |

Countries and modifier source bundles are ordered by numeric ID. Known
technologies are ordered lexicographically. Scheduled grants are ordered by
due tick, country ID, then source ID. These arrays must be strictly ordered
and have no duplicate keys. Modifiers within a source retain their original
order. Whitespace and object field order are accepted on load. Saving again
emits the canonical representation. A readable example is
`crates/data/tests/fixtures/save_v1.json`.

The state hash is SHA-256 over the UTF-8 bytes of `strategy:game-state:v1`,
then a single NUL byte, then the canonical save bytes. Hashing includes the
map identity and schema version. Runtime state in memory uses the integer
and ordered types from `sim`. Callers handle file input/output.

## Countries (`data/countries/<tag>.json`)

M3 country, character, scenario and AI profile files use strict JSON with unsigned
32-bit `schema_version: 1`. Tags and keys are ASCII snake_case strings.
Country and character numeric IDs are assigned in sorted key order starting at 1.

| Country field | Representation and unit |
|---|---|
| `tag` | Country key and filename stem. |
| `name` | Nonempty display name. |
| `color` | Three unsigned 8-bit RGB channel values in the order red, green, blue. |
| `capital` | Province key referring to one of this country's land provinces. |
| `ai_profile` | Profile key. |
| `start.ruler`, `start.heir` | Character keys. The heir may be omitted or null. |
| `start.provinces` | Unique land province keys, owned by one country at most. |
| `start.treasury`, `start.monthly_spending` | Raw Fixed thousandths of a treasury unit. Spending is per game month. |
| `start.tax_rate` | Raw Fixed fraction of taxable income. `250` means 25%. |

### World-map country extensions (planned for M13)

`stub` is a boolean marking a placeholder country. MAP_SCALE.md defines the content requirements for stub countries. The current strict country loader predates this field.

## Characters (`data/characters/<key>.json`)

| Character field | Representation and unit |
|---|---|
| `key`, `country`, `dynasty` | Character filename stem, country tag, dynasty key. |
| `name` | Nonempty display name. |
| `birth`, `death` | Gregorian `[year, month, day]` as in saves. Omit `death` or use null if unspecified. |
| `stats.adm`, `stats.dip`, `stats.mil` | Whole unsigned 8-bit skill points. |
| `traits` | Unique keys from the scenario's trait map. |

## Technologies (`data/techs/<branch>.yaml`) — DAG
`key, branch, cost, requires:[keys], pos:{x,y}, effects:[Effect]`

## Ideas (`data/ideas/<tag>.yaml`)
`key, name, modifiers:[{key,value,kind}], unlock: Trigger`

## Events (`data/events/*.yaml`)
`key, title, desc, trigger: Trigger, mean_time_to_happen?, options:[{text, effects:[Effect], ai_weight}]`

Every option requires an integer `ai_weight`, optionally conditioned by a trigger.

## Scenarios (`data/scenarios/<year>.json`)

| Scenario field | Representation and unit |
|---|---|
| `start_date` | Gregorian `[year, month, day]` as in saves. |
| `seed` | Unsigned 64-bit RNG seed. |
| `countries`, `characters` | Unique country tags and character keys to load. |
| `unowned_provinces` | Optional list of unique land province keys with no starting owner. When supplied, owned and unowned keys must cover every land province exactly once. |
| `human_slots` | Country tag to unique player slot number, 1 or 2. Other countries use their AI profile. |
| `rules.adulthood_years` | Positive unsigned 16-bit whole calendar years. |
| `rules.base_taxable_development_per_province` | Unsigned 32-bit whole development points added per province for economy. |
| `rules.tax_per_development` | Raw Fixed thousandths of a treasury unit per taxable development point per month, before the tax rate. |
| `rules.max_tax_rate` | Raw Fixed fraction from 0 through 1000. |
| `rules.max_monthly_spending` | Nonnegative raw Fixed treasury amount per month. |
| `rules.debt_limit` | Nonnegative raw Fixed treasury amount. The debt floor is its negative. |
| `rules.income_modifier` | Modifier key used to calculate monthly tax income. |
| `traits` | Trait keys mapped to arrays of permanent modifier records in the format below. |
| `military` | Optional object containing `rules` and `forts`. Forts map land province keys to positive unsigned 8-bit levels. |

Validate content and cross-references with
`cargo run --locked -p strategy-data --bin lint-scenario -- data/scenarios/1440.json data/map/western_europe/provinces.json`.

## Trigger and effect syntax

A trigger is a single-key mapping. The country that owns the trigger is its subject.

| Trigger field | Representation and unit |
|---|---|
| `all` | List of triggers. True for an empty list. |
| `any` | List of triggers. False for an empty list. |
| `not` | One trigger. |
| `country_has_tech` | Technology key, as a snake_case string. |
| `at_war` | Boolean. Compares with the subject's current war state. |

Effects are listed in declaration order. The `add_modifier` effect applies
to the subject country. Trusted scenario or event code supplies the source ID.
The simulation calculates expiry in ticks, with one tick per game day.

| Modifier field | Representation and unit |
|---|---|
| `key` | Modifier key, as a snake_case string. |
| `value` | Signed 64-bit integer in raw Fixed thousandths for both modifier kinds. `1000` means `1.000` and `100` means `0.100`. |
| `kind` | `additive` adds a flat amount. `multiplicative` multiplies by `1 + value`. |
| `duration_days` | For `add_modifier` only. Positive whole game days. Omit the field for a permanent source. |

For a flat bonus of +1, write `value: 1000, kind: additive`. For a
10% bonus, write `value: 100, kind: multiplicative`. Numeric fractions and
strings such as `0.1` and `"10%"` are invalid modifier values, including in
ordinary `modifiers` lists.

Install the pinned lint dependencies with `python -m pip install -r tools/requirements.txt`,
then run `python tools/lint_data.py` to check JSON and YAML data.

```yaml
trigger: { all: [ { country_has_tech: mil_3 }, { not: { at_war: true } } ] }
effects: [ { add_modifier: { key: army_morale, value: 100, kind: multiplicative, duration_days: 3650 } } ]
```

Add new leaves here first, then implement.

## Rules
- Localizable strings use keys; text lives in `data/loc/en.yaml`.
- Every file has `schema_version`. Breaking change = bump version + migration note.
- Every schema field declares its representation and unit. Fixed values use raw thousandths. Counts, IDs, versions, pixel coordinates and durations are ordinary integers in their own units, with no Fixed scaling.

## AI profiles (`data/ai/profiles/<key>.json`)

| Profile field | Representation and unit |
|---|---|
| `key` | Profile key and filename stem. |
| `weights.aggression`, `expansion`, `caution`, `diplomacy`, `economy`, `naval`, `loyalty` | Whole weight points from 0 through 1000. |
| `thresholds.war_min_score`, `peace_accept_score`, `alliance_min_opinion` | Signed 32-bit whole decision score points. |
| `preferred_tax_rate` | Raw Fixed fraction within scenario tax limits. |
| `preferred_monthly_spending` | Raw Fixed treasury amount per month within scenario spending limits. |

## Saved games (schema version 2)

A game with countries and characters adds `state.polity` with `rules`,
`countries`, `characters`, and `ai_profiles`. The static map is supplied as
for v1. The loader still reads v1 snapshots for simulations without polity.
The v2 state hash uses `strategy:game-state:v2` followed by NUL and the
canonical save bytes.

| Additional save field | Representation and unit |
|---|---|
| `polity.rules` | Scenario rule fields and units defined above. |
| `polity.countries[].id`, `capital`, `provinces[]` | Unsigned 32-bit country and map province identifiers. |
| `polity.countries[].economy.treasury` | Raw Fixed treasury amount. |
| `economy.policy.tax_rate`, `economy.policy.monthly_spending` | Raw Fixed fraction and treasury amount per month. |
| `economy.last_month` | Null or `{income, expenses}` in raw Fixed treasury amounts. Expenses are the amount paid. |
| `economy.bankrupt` | Boolean bankruptcy flag. |
| `polity.countries[].succession.ruler`, `heir` | Null or unsigned 32-bit character identifiers. |
| `polity.characters[].id`, `country` | Unsigned 32-bit character and country identifiers. |
| `polity.characters[].sources` | Modifier sources with the v1 source, expiry and value units. |
| `polity.ai_profiles[]` | Profile fields and units above without a nested schema version. |

Other country and character metadata keeps the content field representations.
Country, character and province records are sorted by ID, profiles and trait
keys by key, and modifier sources by ID.

## LAN AI presets

LAN AI tuning in `data/ai/lan_presets.json` has unsigned 32-bit
`schema_version: 1` and a `presets` object keyed by `balanced`, `cautious`
and `aggressive`. Each entry contains `aggression_scale`, unsigned
16-bit raw Fixed thousandths between 0 and 2000, and
`war_min_score_adjustment`, signed 32-bit whole score points between
-1000 and 1000. The host scales aggression, caps it at 1000 weight points,
and adds the threshold adjustment when creating a LAN room.

## Military rules

| Rule | Representation and unit |
|---|---|
| `raise_cost_per_soldier` | Raw Fixed treasury amount per soldier raised. |
| `max_army_size`, `base_supply` | Unsigned 32-bit whole soldiers per army and base whole soldiers supplied per province. |
| `supply_per_development` | Unsigned 32-bit whole soldiers supplied per development point. |
| `max_morale`, `retreat_morale_threshold`, `battle_morale_loss` | Raw Fixed morale amounts. |
| `morale_recovery_per_tick` | Raw Fixed morale amount recovered per game day. |
| `winner_casualty_rate`, `loser_casualty_rate`, `attrition_rate` | Raw Fixed fractions from 0 through 1000. `100` means 10%. |
| `siege_days_per_level` | Unsigned 32-bit whole game days per fort level. |
| `battle_score`, `occupation_score`, `peace_score` | Signed 32-bit whole war-score points for a battle, an occupation and the minimum for a cession, respectively. |
| `truce_ticks` | Unsigned 32-bit whole days during which the same pair cannot declare another war. |
| `combat_modifier`, `morale_modifier` | Modifier keys used for battle strength and morale. |

## Saved games (schema version 3)

A game with military rules adds `state.military` to the v2 payload. V3
uses the hash domain `strategy:game-state:v3` followed by NUL and the
canonical save bytes. V1 and v2 remain readable with their original map
identities. The weighted map has a new identity and does not migrate older
saves automatically.

| Additional save field | Representation and unit |
|---|---|
| `military.rules` | Military rules as specified above. |
| `next_army_id`, `next_war_id` | Next positive unsigned 32-bit identifiers. |
| `armies[].id`, `country`, `province` | Unsigned 32-bit army, country and province identifiers. |
| `armies[].soldiers`, `morale` | Whole unsigned 32-bit soldiers and raw Fixed morale. |
| `armies[].movement` | Null or `{route, elapsed_ticks}`. The route is ordered `{to, kind, cost}` edges, with province ID, edge type and positive whole tick cost. `elapsed_ticks` is unsigned 32-bit days into the first edge. |
| `wars[].id`, `attacker`, `defender`, `goal` | Unsigned 32-bit war, country and goal province identifiers. |
| `wars[].started_at`, `score` | Unsigned 64-bit tick and signed 32-bit score points from the attacker's perspective. |
| `wars[].occupations` | Sorted `{province, country}` pairs of unsigned 32-bit occupied province and occupying country IDs. |
| `wars[].offer` | Null or `{offered_by, cede_goal}`, an unsigned 32-bit country ID and a boolean. |
| `forts[].province`, `level` | Unsigned 32-bit province ID and positive unsigned 8-bit fort level. |
| `sieges[].province`, `war`, `besieger`, `progress_days` | Unsigned 32-bit province, war and country IDs, and whole days of progress. |
| `treaties[].war`, `attacker`, `defender`, `ceded`, `concluded_at` | Unsigned 32-bit war, country and ceded province IDs, then an unsigned 64-bit tick. `ceded` is null for white peace. |

Armies and wars are sorted by ID, forts and sieges by province, and
treaties by war ID. An active movement route must match the loaded graph.
