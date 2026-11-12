# AI (bot-controlled countries)

Every country not controlled by a human is played by AI. Humans: 2 (host + friend). AI plays the rest: economy, armies, diplomacy, events, succession, tech.

## Principles
1. **AI is a player.** It acts only by producing `Command`s that go through the same validation and `apply` as human commands. AI code never mutates `GameState` directly.
2. **Host only.** AI runs on the host inside the tick loop. Clients never run AI; they receive deltas.
3. **Deterministic.** Same state + seed => same AI commands. AI uses an RNG derived from `(state.seed, tick, country_id)`, never wall-clock time, never `f32/f64`, never unordered iteration.
4. **Data-driven.** Weights and personalities live in data files, not code (DATA_FORMATS.md, "AI profiles").
5. **No hidden cheats.** In beta the AI reads the full state (no fog of war, DECISIONS D9) but gets no free resources. Any difficulty bonus must be an explicit modifier (e.g. `ai_income_modifier`) shown in debug UI.
6. **Explainable.** Every AI command carries a `reason` code; a debug ring buffer keeps the last N decisions per country ("why did France declare war?").
7. **Cannot crash the game.** AI errors/panics are caught per country; on failure that country does nothing this tick and the error is logged.

## Structure
- Crate `crates/ai`, depends on `sim` (read-only access to `GameState` via query helpers). `sim` never depends on `ai`.
- Entry point: `ai::decide(state: &GameState, country: CountryId, ctx: &mut AiContext) -> Vec<(Command, Reason)>`.
- Host tick: after scheduled events, for each AI country (ascending `CountryId`) whose turn it is, call `decide`, validate and apply the returned commands (see SIM_RULES.md tick order).
- Country control: `controller: Human(player_id) | Ai(profile_key)`, stored in `GameState`, set from the lobby/scenario. Changing it is a host-only command.

The host supplies the decider to the simulation's ordered callback with a
fresh `AiContext` for each country and tick. It catches and logs a panic for
the affected country and continues dispatching. A game without polity
retains the M1 stub behavior.

L0 checks its budget every 28 ticks, staggered by country ID. It prefers
the taxes and spending from its profile when the next budget is solvent.
If debt is projected or already present, it raises taxes to the scenario
cap and suspends spending until it can afford its preferred budget again.
When a valid heir is absent, it picks the oldest eligible character of
the ruler's dynasty, then the oldest remaining eligible character, with
character ID breaking age ties. The host retains the last 128 decisions
per country with their command and reason.

L1 schedules military orders every three ticks and checks war targets when
that schedule coincides with its 31-tick strategy schedule. It raises at its capital when it has no
army, compares reachable neighbors' strength, and scores declarations
with aggression, expansion and caution from its profile. The derived
country RNG contributes a bounded tie-breaking adjustment. At war it
defends a threatened capital or the war goal, concentrates supplied
armies, splits oversized armies, and retreats when outnumbered with low
morale. It offers or accepts cession according to occupation, war score
and relative strength. A two-year stalemate allows white peace. Every
military order includes a score and reason in the per-country decision log.
In a war with supporters, L1 treats all opposing participants as enemies,
moves and retreats supporter armies using the same validated commands, and
leaves peace proposals and acceptance to the original war leaders.
Automatic post-war truces block AI war declarations through the same
`DeclareWar` validation used for human players; AI does not propose a separate
peacetime truce.

The first L2 alliance slice answers pending offers through `Command`s. A
nonnegative opinion at or above the profile's `alliance_min_opinion` accepts.
With negative opinion, both countries must border the same third country
through a normal or river edge, and that country's total active soldiers must
strictly exceed each party's total; the derived country/tick RNG then gives a
10% acceptance chance. Without that threat, AI rejects. AI proposes legal
alliances on a staggered 31-tick cadence. The read-only factor query reports
opinion, profile minimum, common threat and direct war for human tooltips.
On its 31-tick diplomacy cadence, an AI can guarantee a non-allied country
when its opinion meets its profile's alliance minimum and both countries
border a stronger third country. The AI issues only the validated guarantee
command. Entering a defensive war follows the guarantee automatically; AI
military orders then use the existing supporter-side rules. AI does not revoke
guarantees periodically.

M7 event choices are read from the authoritative pending-event state. For each
AI country the host chooses at most one pending event per tick in event-key
order. The AI adds the one conditional weight when its trigger is true, clamps
negative results to zero and falls back to base weights if all adjusted
weights are zero. A keyed seeded draw selects the option, and the AI submits a
`ChooseEvent` command through the ordinary validation path. It never changes
the pending event directly.

Royal-marriage AI uses a separate read-only factor list: effective opinion
toward the proposer, `marriage_min_opinion`, and legal named-character pair.
It accepts a legal offer when effective opinion meets the data profile's
threshold, otherwise rejects. Human acceptance has no opinion threshold.
On the same staggered 31-tick L2 cadence, an AI may offer a legal marriage
to the highest-opinion country; stable character ID order breaks ties. This
choice uses no random draw. The current 1440 content has no eligible woman,
so the behavior is covered with a synthetic scenario until M11 content work.

Run the same host tick without a window with
`cargo run --locked -p strategy-app --bin ai-smoke -- 3653 1440 1441 1442`.
The arguments are the daily tick count followed by one or more seeds.

At war, the L3 naval planner prefers a legal amphibious order. Otherwise it
sends an idle uncargoed fleet through the validated `MoveFleet` command toward
an enemy port it can blockade. A fleet already maintaining a blockade stays
in its sea zone. The decision log uses `BlockadePort` as its reason code.

For yearly multi-seed measurements, run
`cargo run --locked -p strategy-app --bin ai-soak -- 100 1440 1441 1442 1443 1444`.
It prints tab-separated rows with state hashes, ownership share and local
median/p95/peak tick times. The opening includes the packaged event catalog
and diplomacy state. Timing varies by host and is excluded from state hashes.

## Decision layers (utility scoring, not machine learning)
| Layer | Cadence | Decides |
|-------|---------|---------|
| Strategic | monthly | threats, war targets, alliances, who to befriend, peace stance, expansion goals |
| Economy | monthly | taxes, spending, building, tech/idea picks, debt avoidance |
| Military | every few days | raise/merge armies, assign targets, sieges, retreat, defend capital |
| Naval | every few days | fleets, transports, escort, blockade |
| Diplomacy responses | on proposal | accept/reject by opinion formula (same formula used when a human proposes to AI) |
| Events | on event | pick option by `ai_weight` + profile |
| Ruler | on trigger | marriages, heirs, regency choices |

The first technology AI slice runs on the first day of each calendar month.
It chooses one affordable technology whose prerequisites are all known, using
the profile's `tech_branch_weights`; equal scores use ascending technology key.
It submits `UnlockTechnology` through the same validation as a human. The
separate M9 needs-aware choice slice will add war/economy factors.

Each layer scores candidate actions: `score = sum(weight_i * factor_i)` with factors computed from state (strength ratio, distance, opinion, war score, treasury, threats). Highest score above a threshold wins; ties broken by `CountryId`/stable order then RNG.

## Scheduling and budget
- Staggered: country `c` runs layer `L` on ticks where `(tick + c) % period(L) == 0`, so not all AIs think on the same day.
- Budget in **operations**, not milliseconds (keeps determinism): cap candidate evaluations per country per tick. Tune caps with the benchmark.
- Max commands per country per tick (e.g. 5) to prevent command spam.
- Tick time with all AI countries must stay inside the budget in ARCHITECTURE.md.

## Capability levels (mapped to milestones)
- **L0 (M3)**: manages economy and succession; does nothing else.
- **L1 (M4)**: raises and moves armies, defends, attacks weaker neighbors, runs sieges, accepts/proposes peace by war score.
- **L2 (M6)**: diplomacy — alliances, guarantees, marriages, truces, reacts to proposals and to threats.
- **L3 (M7, M8, M9)**: chooses event options, uses navy and transports, picks technologies and ideas.
- **L4 (M10)**: strategic coordination, balance of power, alliance calls, long-term goals, tuning, soak testing.

The first M8 naval slice follows an existing attacking war across sea. On its
three-day military cadence, the AI validates the same commands as a human:
move an army to an owned port, embark it on a fleet with enough ships, sail
to a sea cell adjacent to the war goal, and land at the enemy port. It does
not issue a command when no legal route or capacity exists. During naval
combat, it uses the same validated fleet move command and avoids a sea route
where an opposing fleet has at least as many ships as its own fleet. Declaring
a new overseas war, escorts, blockade and attrition avoidance remain
part of the later M8 L3 work.

Every mechanic milestone must ship its AI level in the same milestone (see each milestone's "AI slice"). A mechanic without AI support is not accepted.

## Profiles and personalities
Per-AI-country profile with weights such as `aggression`, `expansion`, `caution`, `diplomacy`, `economy`, `naval`, `loyalty` (to allies). Profiles are data; countries reference a profile key, with optional per-country overrides. Add a few archetypes (aggressive, defensive, mercantile, balanced) first, then refine.

## Human/AI interaction
- A human proposal to an AI country is answered by the same opinion/acceptance formula the AI uses; the UI shows the formula's factors.
- Disconnected human: country is held (no AI) for a host-defined grace period, then the host may switch it to AI temporarily (DECISIONS D8). Reconnect returns control to the human.
- Lobby: host picks which countries are human-controlled and the AI aggressiveness preset.

## Debugging tools (required by L1)
- Debug panel: select an AI country -> last decisions with reason codes and scores.
- Dev command to run N ticks headless with all countries on AI (`sim` + `ai`, no UI).

## Testing
- **Unit**: each scoring function with fixed states.
- **Scenario tests**: AI accepts a clearly winning peace; does not declare war on a stronger coalition; defends its capital; does not send armies into unreachable provinces; never issues an invalid command.
- **Determinism**: AI-only run with a fixed seed -> identical state hash across runs and after save/load.
- **Soak (from L1, extended each level)**: AI-only run of 100+ game years across several seeds. Assert: no panic, no invariant violation, tick time in budget, not everyone dies by year X, not one country swallows the map too early. Record summary stats (wars per decade, country count over time) in DECISIONS.md.
- **CI gates**: AI smoke test (10 game years, 3 seeds) from M4; soak test as a scheduled/nightly or manual workflow from M10. Reference in GIT_WORKFLOW.md when enabling.

## Non-goals (beta)
No neural networks, no LLM-driven decisions in the game loop, no fog-of-war-aware AI, no AI cheating by default, no perfect play. The goal is believable, stable opponents.
