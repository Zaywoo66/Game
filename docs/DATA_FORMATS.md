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

Schema v3 also requires an exact land-to-sea port correspondence: every sea
ID in a land province's `ports` must have a `port` edge in both directions,
and every `port` edge must connect land to sea with its sea ID listed in the
land province's `ports`. Sea and lake provinces have empty `ports` lists.
The stricter correspondence does not apply to legacy v1/v2 maps, preserving
their save-file map identities. Sea routes use only sea-to-sea `normal` and
`strait` edges; route cost is the sum of whole daily movement ticks. `port`
and `river` edges do not extend a sea route.

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
| `data/map/<region>/names.en.json` | Flat v1 localization key→English fallback name mapping, with no `schema_version` field. |

MAP_SCALE.md defines the hierarchy and reserved ID ranges. The generated Western Europe map uses `map_version: 3`. The loader still verifies schema v1/v2 maps with their original hash rules. Older saves are accepted only when loaded against their own map identity.

### 1440 polity overlap evidence (`polities_1440.json`)

This generated sidecar records the largest rasterized Cliopatria v0.1.3 polygon
overlap for every land province. It is evidence for content review, not a
scenario owner table. An unmatched cell stays unknown; Holy Roman Empire minor
states and Kalmar Union labels are coarse groups. Equal pixel overlaps break
by polity name in lexical order.

| Field | Representation and unit |
|---|---|
| `schema_version` | Integer sidecar schema version, currently 1; unitless. |
| `map_version` | Positive integer map version matching `provinces.json`; unitless. |
| `map_hash` | Lowercase SHA-256 hex string matching `provinces.json`. |
| `reference_year` | Integer Gregorian calendar year, currently 1440. |
| `source` | `name`, `release`, `url`, `license`, repo-relative `file`, and lowercase SHA-256 `sha256` of the clipped GeoJSON with Git-canonical LF line endings. |
| `method` | Literal raster overlap selection and tie break. |
| `assignments[].province_id` | Positive integer province ID from the map; unitless. |
| `assignments[].province_key` | Stable province key from the map. |
| `assignments[].polity_name` | Exact source polygon name, or `null` when no polygon overlaps. |
| `assignments[].resolution` | `polygon`, `coarse_group`, or `unmatched`; a precision label, not ownership. |
| `assignments[].overlap_pixels` | Nonnegative integer raster pixels of the selected polity within the province; zero when unmatched. |
| `assignments[].province_pixels` | Positive integer raster pixels in the province. |

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

Country, scenario and AI profile files use strict JSON with unsigned 32-bit
`schema_version: 1`; characters use version 2 as specified below. Tags and keys are ASCII snake_case strings.
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

The UI loads an optional licensed SVG from `ui/src/assets/flags/<tag>.svg`,
where `<tag>` is the country's `tag` in this file. This is a static content
asset convention; missing SVGs leave the country color swatch visible. Record
each new SVG and its source in `docs/ASSETS.md`.

### World-map country extensions (planned for M13)

`stub` is a boolean marking a placeholder country. MAP_SCALE.md defines the content requirements for stub countries. The current strict country loader predates this field.

## Characters (`data/characters/<key>.json`)

Character schema v2 makes `sex` mandatory. Schema v1 character content is
rejected during scenario loading; old saved games remain readable as described
below.

| Character field | Representation and unit |
|---|---|
| `key`, `country`, `dynasty` | Character filename stem, country tag, dynasty key. |
| `name` | Nonempty display name. |
| `sex` | Required `male` or `female`. This content value is checked against biographical sources for historical characters. |
| `spouse` | Optional key of another character in the scenario. Both character files must point to each other; the pair must be living, adult, from different countries and of opposite sex at scenario start. Omission means no modeled spouse. |
| `birth`, `death` | Gregorian `[year, month, day]` as in saves. Omit `death` or use null if unspecified. |
| `stats.adm`, `stats.dip`, `stats.mil` | Whole unsigned 8-bit skill points. |
| `traits` | Unique keys from the scenario's trait map. |

The 13 existing `male` entries were checked against the [UK Royal Family biographies](https://www.royal.uk/henry-vi) (Henry VI, Edward IV, Humphrey of Gloucester, James II and James III), [France's Larousse biographies](https://www.larousse.fr/encyclopedie/personnage/Charles_VII/112825) (Charles VII and Louis XI), the [Spanish national archives](https://pares.cultura.gob.es/ParesBusquedas20/catalogo/autoridad/46221) (John II of Aragon, Alfonso V of Aragon, John II and Henry IV of Castile), and the [Portuguese expansion encyclopedia](https://eve.fcsh.unl.pt/en/people/prince-dom-fernando-1433-1470) (Ferdinand of Viseu and Afonso V of Portugal). No female historic character has been added to the 1440 slice; that content review belongs to M11.

## Technologies (`data/techs/<branch>.yaml`) — DAG

Technology branch files use YAML with `schema_version: 1`, a `branch` key
equal to the ASCII snake_case filename stem, and a `technologies` list. Every
technology key is unique across all branch files. Prerequisites may refer to
another branch, but every referenced key must exist and the complete directed
graph must have no cycles. The data linter checks those conditions before
technology content enters the game. Research points accrue by 10 on each new
calendar month and are capped at 1000 whole points. `UnlockTechnology` spends
the data cost at once if all prerequisites are known. Technology modifiers are
permanent and have a source record keyed by country and technology.

| Field | Representation and unit |
|---|---|
| `schema_version` | Integer `1` for the branch file. |
| `branch` | ASCII snake_case branch key, equal to the filename stem. Branch names are content keys, not a fixed list in code. |
| `technologies[].key` | Globally unique ASCII snake_case technology key. |
| `technologies[].cost` | Positive unsigned 32-bit integer in whole research points. |
| `technologies[].requires` | List of unique technology keys; an empty list denotes a root node. |
| `technologies[].pos.x`, `pos.y` | Signed 32-bit integers in tech-tree layout grid units. |
| `technologies[].effects` | List of permanent `add_modifier` effects in the Trigger and effect syntax section. Modifier `value` uses raw Fixed thousandths; `duration_days` is forbidden for technology effects. |

For example:

```yaml
schema_version: 1
branch: military
technologies:
  - key: professional_officers
    cost: 250
    requires: [recordkeeping]
    pos: {x: 2, y: -1}
    effects:
      - add_modifier: {key: army_morale, value: 100, kind: multiplicative}
```

Run `python tools/lint_data.py` to check this contract. It rejects missing or
duplicate keys, duplicate branch files, repeated prerequisites, unknown
fields/effects, invalid units, and cycles. The parser applies the same
canonical YAML scalar rules as events:
lowercase booleans and decimal integer literals without separators or leading
zeros.

`strategy_data::parse_technology_branch_yaml` converts one branch to typed
definitions with whole research-point costs, signed layout positions, and
simulation `Effect` values. `strategy_data::load_technologies_from_dir`
reads `.yaml`/`.yml` files in filename order, returns a technology-key ordered
map, and rejects duplicate branch files, duplicate technology keys, missing
prerequisites and cycles. The Rust loader uses the existing pinned
`yaml_serde` 0.10.7. The research runtime and profile-weighted AI choice are
recorded below; ideas and reforms remain separate M9 tasks.

The desktop tech tree reads the validated packaged layout coordinates from
these YAML files. Coordinates are presentation grid units and are not saved;
research points, known technologies, costs, prerequisites and modifier effects
are projected from the authoritative game state. The UI receives the catalog
in its initial snapshot and receives changed country research rows through
normal host updates. `data/locales/techs.en.json` is a flat English text map:
`branch.<branch_key>` names every branch, `tech.<technology_key>` names every
technology, and `modifier.<modifier_key>` names each technology effect. The
data linter requires nonempty entries for every referenced key and rejects
duplicate translation keys.

## Ideas (`data/ideas/<tag>.yaml`)
`key, name, modifiers:[{key,value,kind}], unlock: Trigger`

## Events (`data/events/*.yaml`)

Each YAML file contains one event. The linter rejects duplicate event keys or
YAML mapping fields, unknown trigger/effect names, missing AI weights, and
noninteger durations or weights. Boolean literals in event YAML use lowercase
`true` or `false` only; the linter rejects YAML 1.1 boolean spellings such as
`yes` that the Rust loader does not accept. Event integer literals use plain
decimal digits, with an optional leading minus for signed fields; leading
zeros, separators, plus signs and hexadecimal forms are rejected so both
parsers read the same value. This is the M7 data contract; the event engine is
implemented in a separate M7 task.
`strategy_data::parse_event_yaml` converts one file to typed `Trigger` and
`Effect` values; `strategy_data::load_events_from_dir` reads files in filename
order and returns a key-ordered map, rejecting duplicate event keys. Text
fields remain localization-key references; the data linter checks their presence
in `data/locales/events.en.json`. The Rust loader uses pinned
`yaml_serde` 0.10.7.
The authoritative game copies these validated definitions into its state. On
each game day it evaluates definitions in event-key order and countries in
numeric ID order. The keyed seeded draw gives each eligible country-event pair
a daily probability of `1 / mean_time_to_happen`. A pending choice suppresses
duplicates. One-shot events never fire again after resolution; repeatable
events wait `cooldown_days` from the resolution tick. The option index is
zero-based. Each effect receives a unique monotonically increasing modifier
source ID so timed modifiers can expire independently.

The event locale catalog is a flat JSON mapping from ASCII snake_case keys to
nonempty English display strings. Every `title`, `desc`, and option `text` key
used by a valid event must be present. Duplicate keys, empty strings, and
missing references fail data lint. Vite serves the English catalog at
`/locales/events.en.json` in development and includes it in production UI
builds. The popup UI consumes this route in a separate M7 task; other
languages remain open.

| Field | Representation and unit |
|---|---|
| `key` | Unique ASCII snake_case event key; unitless. |
| `title`, `desc` | ASCII snake_case localization keys, not display text. |
| `trigger` | One `Trigger` mapping from the supported syntax below. Evaluated for the subject country. |
| `mean_time_to_happen` | Optional positive unsigned 64-bit integer in game days. While the trigger is true, each day has probability `1 / mean_time_to_happen` using a deterministic draw keyed by seed, tick, country ID and event key. Omission means no random automatic firing. |
| `repeatable` | Optional Boolean, default `false`. A nonrepeatable event can fire once per country. |
| `cooldown_days` | Positive unsigned 64-bit game days, required only when `repeatable: true`; counts after resolution. A pending choice never duplicates. |
| `options` | Nonempty ordered list of option mappings. At least one base AI weight must be positive. |
| `options[].text` | ASCII snake_case localization key. |
| `options[].effects` | Ordered list of supported `Effect` mappings below; may be empty. |
| `options[].ai_weight` | Required nonnegative unsigned 32-bit integer weight, unitless. |
| `options[].ai_weight_if` | Optional `{when: Trigger, add: signed i32 integer}`. When true, add to the base weight and clamp below at zero. If every adjusted option weighs zero, AI uses the base weights. |

Example:

```yaml
key: harvest_festival
title: event_harvest_title
desc: event_harvest_desc
trigger: {not: {at_war: true}}
mean_time_to_happen: 365
repeatable: true
cooldown_days: 365
options:
  - text: event_harvest_accept
    ai_weight: 10
    ai_weight_if: {when: {country_has_tech: agrarian_reform}, add: 5}
    effects:
      - add_modifier: {key: harvest_tax, value: 100, kind: multiplicative, duration_days: 30}
  - text: event_harvest_decline
    ai_weight: 1
    effects: []
```

The 1440 starter pack currently has three generic events in `data/events/`:
`court_auditors`, `harvest_surplus`, and `frontier_patrol`. They use only the
current `at_war` trigger and existing `tax_income`, `army_combat`, and
`army_morale` modifier keys. This content is not a historical claim.

## Scenarios (`data/scenarios/<year>.json`)

| Scenario field | Representation and unit |
|---|---|
| `start_date` | Gregorian `[year, month, day]` as in saves. |
| `seed` | Unsigned 64-bit RNG seed. |
| `countries`, `characters` | Unique country tags and character keys to load. |
| `unowned_provinces` | Optional list of unique land province keys with no starting owner. When supplied, owned and unowned keys must cover every land province exactly once. |
| `human_slots` | Country tag to unique player slot number, 1 or 2. Other countries use their AI profile. |
| `opinions` | Optional directed records `{from, toward, points}`. `from` and `toward` are distinct country tags in `countries`; `points` is a signed integer in whole opinion points from -100 through 100. Pairs must be unique. Missing pairs have opinion 0. |
| `rules.adulthood_years` | Positive unsigned 16-bit whole calendar years. |
| `rules.base_taxable_development_per_province` | Unsigned 32-bit whole development points added per province for economy. |
| `rules.tax_per_development` | Raw Fixed thousandths of a treasury unit per taxable development point per month, before the tax rate. |
| `rules.max_tax_rate` | Raw Fixed fraction from 0 through 1000. |
| `rules.max_monthly_spending` | Nonnegative raw Fixed treasury amount per month. |
| `rules.debt_limit` | Nonnegative raw Fixed treasury amount. The debt floor is its negative. |
| `rules.income_modifier` | Modifier key used to calculate monthly tax income. |
| `traits` | Trait keys mapped to arrays of permanent modifier records in the format below. |
| `military` | Optional object containing `rules` and `forts`. Forts map land province keys to positive unsigned 8-bit levels. |
| `navy.fleets[]` | Optional starting fleets. Each record has `country` (country tag), `port` (owned land province key), `sea` (adjacent sea province key with a reciprocal `port` edge), and `ships` (positive unsigned 32-bit count). Records receive stable numeric fleet IDs in array order, starting at 1. `navy` requires `military`. |

M8.2 has one transport ship type. Each ship holds 1,000 whole soldiers; an
army boards as one unit and must fit in one fleet. Fleets start in the
specified sea cell, travel only by `normal` or `strait` sea-to-sea edges, and
spend each edge's positive whole tick cost in game days. Wartime opponents
fight when they share a sea cell. One battle per war and sea cell resolves each
day, sorted by war ID then sea ID. Each side sums its ships. The larger side
wins, losing `ceil(ships * 10 / 100)` while the loser loses
`ceil(ships * 25 / 100)`. A tie costs both sides 10%. Losses consume fleets
by ascending fleet ID. Losers, or both sides in a tie, cancel routes and
retreat to their previous safe sea cell, otherwise the adjacent safe cell
with the lowest ID. A fleet with no safe cell remains until the next day.
Zero-ship fleets are removed. An embarked army is lost if its fleet sinks or
the surviving ships cannot carry its full strength. Disembarkation is blocked
while an opposing wartime fleet occupies the same sea cell; the host returns
the blocking fleet ID. Peaceful fleets may share
a sea cell. Attrition, blockades and ship construction follow in separate M8/M9
tasks. Peacetime alliance alone grants no port access. Embarkation requires a
port owned by the country or a participant on its side of the same war;
landing may use such a port or an enemy port during war, with land combat
resolved on the next tick. An embarked army keeps its last land province in
the army record for save compatibility, but is excluded from land movement,
supply, sieges and battles until landing. Its actual position is its carrier
fleet's sea cell.

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
| `thresholds.war_min_score`, `peace_accept_score`, `alliance_min_opinion`, `marriage_min_opinion` | Signed 32-bit whole decision score points. The 1440 marriage threshold is 20 effective opinion points. |
| `preferred_tax_rate` | Raw Fixed fraction within scenario tax limits. |
| `preferred_monthly_spending` | Raw Fixed treasury amount per month within scenario spending limits. |
| `tech_branch_weights` | Optional object from loaded technology branch key to whole preference points 0 through 1000. Unknown branches and out-of-range weights are rejected when the catalog is attached. Missing branch weights score zero. |

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

## Saved games (schema version 4)

An explicitly configured diplomatic opinion adds `state.opinions`, a canonical
array of `{from, toward, points}` records sorted by numeric `from` and `toward`
country ID. IDs are positive unsigned 32-bit country identifiers; `points` is
signed whole opinion points from -100 through 100. The two countries must be
distinct and present in the state. Duplicate or unsorted pairs are rejected.
An absent pair has opinion 0. A v4 save requires at least one explicit pair;
states without explicit opinions retain their previous save version and bytes.
The v4 state hash uses `strategy:game-state:v4`, a NUL byte, then canonical
save bytes. Opinion does not decay automatically.

## Saved games (schema version 5)

The first alliance offer creates `state.diplomacy` with three sorted arrays:
`alliances` (`{a,b}`), `offers` (`{a,b,until}`), and `cooldowns`
(`{a,b,until}`). Country IDs `a` and `b` are positive unsigned 32-bit IDs.
Alliance and cooldown pairs are ordered with `a < b`; offers are directed
from `a` to `b`. `until` is a Gregorian `[year,month,day]` date. Offers are
valid before `until`; cooldowns prohibit new offers before `until`.
Each pair must exist in the saved countries. Duplicate or unsorted records,
self-pairs, expired timers, simultaneous alliance and offer/cooldown, and
opposing simultaneous offers are rejected. A state with diplomacy uses
schema v5 and hash domain `strategy:game-state:v5` followed by NUL and the
canonical bytes; older versions retain their prior encoding and hash.
An active alliance between two countries directly at war is invalid and
rejected on load.

## Saved games (schema version 6)

An active war with supporters or a completed coalition truce uses save schema
v6 and hash domain `strategy:game-state:v6` followed by NUL and canonical
save bytes. Earlier states retain their previous version and byte encoding.
The `wars[]` records gain optional `attacker_supporters` and
`defender_supporters` arrays of positive unsigned 32-bit country IDs, sorted
ascending without duplicates. Leaders must not appear in either array, a
country cannot join both sides, and each country can be in only one active
war. Empty arrays are omitted on save.

`military.coalition_truces` is an optional sorted array of `{a,b,concluded_at}`
records, where `a` and `b` are positive unsigned 32-bit country IDs with
`a < b`, and `concluded_at` is an unsigned 64-bit game tick no later than the
saved tick. Each record represents a former opposing pair other than the
original leaders. Empty arrays are omitted. The original leader pair stays
in `military.treaties`. A peace creates a truce for every opposing pair, all
using the same `military.rules.truce_ticks` duration. V1 through v5 saves
remain readable with their original map identity.

## Saved games (schema version 7)

V7 uses hash domain `strategy:game-state:v7` followed by NUL and canonical
save bytes. New scenario states with known character sex use v7 even before a
marriage is offered. Older saves keep their earlier version and hash.

New character snapshots store `sex` (`male` or `female`) and `spouse` (omitted
when unmarried, otherwise another character ID). Spouse links are reciprocal,
cross-country, opposite-sex, living and adult on the saved date. Pending
marriage offers are directed character-ID pairs with a Gregorian expiry, valid
for 30 game days. A marriage temporarily adds 20 points to each country's
effective opinion, clamped to the normal opinion range; it never changes base
opinion. Character death clears both spouse links. Old v1–v6 saves lacking
`sex` load with internal `Unknown`. Mixed v7 saves may omit `sex` for these
legacy characters while storing known values on others. Unknown characters
cannot enter a new royal marriage. New scenario content must always provide
`male` or `female`. The first
1440 slice has no female character entry, so historical marriage content awaits
M11.

## Saved games (schema version 8)

V8 uses hash domain `strategy:game-state:v8` followed by NUL and the canonical
save bytes when a validated event catalog is attached. `state.events` embeds
definitions in event-key order, pending and resolved entries ordered by
numeric country ID then event key, and `next_source_id` as a positive unsigned
32-bit modifier-source sequence. Each pending or resolved entry records a
country ID, event key and unsigned 64-bit game tick no later than the saved
tick. A pending repeatable event may coexist with its earlier resolution only
after the configured cooldown. Invalid references, order, source IDs and
version/payload combinations are rejected on load. The embedded definitions
include localization keys, triggers, effects, option weights and timing; a
running save therefore keeps its event rules if external YAML later changes.
Saves v1–v7 remain readable without event state. LAN protocol v6 transports
the same canonical snapshot and the validated zero-based event choice command.

## Saved games (schema version 9)

An active defensive guarantee uses save schema v9 and hash domain
`strategy:game-state:v9` followed by NUL and canonical save bytes. The
`state.diplomacy.guarantees` array contains directed `{a,b}` records: `a` is
the positive unsigned 32-bit guarantor country ID and `b` is the protected
country ID. Both IDs must exist and differ. Records are sorted by `(a,b)`
without duplicates. Empty arrays are omitted. A v9 save must contain at
least one guarantee; v1–v8 saves may not carry guarantees. V9 may also carry
the v7 marriage and v8 event payloads, which remain validated and retained
on load. LAN protocol v7 transports the same canonical snapshot and the
validated guarantee/revoke commands.

## Saved games (schema version 10)

A game with attached navy uses save schema v10 and hash domain
`strategy:game-state:v10` followed by NUL and canonical save bytes. The
`state.navy` object contains `fleets` sorted by positive numeric `id` and
`next_fleet_id` (positive unsigned 32-bit, greater than every stored ID).
Each fleet stores `country` (positive country ID), `sea` (positive sea province
ID), `ships` (positive whole unsigned 32-bit ship count), `movement` (null or
`{route,elapsed_ticks}`), and `cargo` (null or a positive army ID). Route
legs are ordered `{to,kind,cost}` with a sea destination ID, `normal` or
`strait` edge kind, and positive whole daily tick cost; `elapsed_ticks` is
nonnegative and smaller than the first leg's cost. The referenced army must
belong to the same country, be stationary and fit at 1,000 whole soldiers per
ship; an army cannot appear in two cargo slots. V10 requires a navy payload
and may retain the prior marriage, event and guarantee payloads. V1–v9 saves
remain readable without navy. LAN protocol v8 carries v10 snapshots and
validated fleet, embark and disembark commands.

## Saved games (schema version 11)

V11 adds `state.technology` with technology definitions ordered by key,
research-point records ordered by country ID, modifier sources ordered by
country ID then technology key, and the positive next modifier-source ID.
Research points are whole unsigned 32-bit points from 0 through 1000. Each
definition stores its branch key, positive whole-point cost, prerequisite
keys, and permanent modifier effects in raw Fixed thousandths. The country
record's `known_technologies` and the technology source records must agree.
The hash domain is `strategy:game-state:v11` followed by NUL and canonical
save bytes. Older saves remain readable without research state.
