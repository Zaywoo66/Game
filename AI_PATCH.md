# AI integration patch — apply to shared docs
Apply these edits in a PR `docs/ai-integration` together with the new/updated files (docs/AI.md, milestones M01, M03–M12). Do not overwrite statuses or measurements already recorded in ROADMAP.md / DECISIONS.md; only add or change what is listed. The project owner explicitly authorizes the AGENTS.md edit below.

## AGENTS.md
1. Line "Players: 2 (host + friend)." -> "Players: 2 humans (host + friend); every other country is AI-controlled."
2. Add row to the "Which doc to open" table: `| AI / bots, country controllers | docs/AI.md |`
3. Add to "Hard rules": "- AI acts only through `Command`s validated like human commands; AI code lives in `crates/ai` and never mutates state directly. A mechanic without AI support is not accepted."

## docs/ARCHITECTURE.md
1. Layout: add `/crates/ai    bot decision logic (reads sim state, emits Commands)`.
2. Dependency direction -> `app -> net, data, sim, ai`; `ai -> sim`; `net -> sim` (types only); `data -> sim`; `sim -> nothing`.
3. Add section "AI": host-only, deterministic, acts through Commands, see docs/AI.md. Countries have `controller: Human | Ai`.

## docs/SIM_RULES.md
1. Replace the "Tick order" list with: 1. Date advance 2. Scheduled events/expiries 3. AI decisions (host calls `ai::decide` per AI country in ascending CountryId; resulting Commands validated and applied) 4. Characters (age, death, traits) 5. Economy 6. Diplomacy timers 7. Armies move 8. Combat/sieges 9. Navy 10. Events/triggers evaluation 11. Emit deltas.
2. Add section "AI support": query helpers are read-only; AI RNG = derived from (seed, tick, country_id); no new sim mechanic is accepted without the query helpers and legal-command checks AI needs.

## docs/DATA_FORMATS.md (append)
```
## AI profiles (data/ai/profiles/<key>.yaml)
key, weights: { aggression, expansion, caution, diplomacy, economy, naval, loyalty }  # integers 0..1000
thresholds: { war_min_score, peace_accept_score, alliance_min_opinion }

## Country AI fields (in data/countries/<tag>.yaml)
ai: { profile: <profile_key>, overrides: { <weight>: <int> } }

## Scenario controllers (data/scenarios/<year>.yaml)
controllers: { <tag>: human | ai }   # human slots are assigned to players in the lobby

## Events
every option requires ai_weight (integer, optionally conditioned by Trigger)
```

## docs/NETWORK.md (append)
```
## Lobby and controllers
Host chooses which countries are human-controlled; all others are AI. AI runs only on host; clients never simulate AI.
Messages: Lobby{countries:[{tag,controller}]}, ClaimCountry{tag}, SetController{tag,controller} (host only).
Disconnect: country is held for a grace period, then host may set temporary AI control; reconnect restores the human (DECISIONS D8).
```

## docs/DECISIONS.md
Add to Open: D8 (disconnect policy: ASSUMED hold, then host may enable temporary AI), D9 (AI sees full state in beta, no fog of war: ASSUMED), D10 (AI difficulty cheats: ASSUMED none by default; only explicit modifiers).
Add to Decided: D11 AI = crate `ai`, host-only, acts through validated Commands, utility-scoring, data-driven profiles, deterministic RNG derived from (seed, tick, country_id).

## docs/ROADMAP.md
Change M10 title to "AI strategic layer, balance, tuning (L4)". Add note under the table: "AI levels L0–L3 are delivered inside M3, M4, M6, M7–M9 (see each milestone's AI slice); M10 only completes L4." Do not change existing statuses.

## docs/GIT_WORKFLOW.md (when enabling gates)
AI smoke test (10 game years, 3 seeds) becomes a required CI gate at M4; soak test is a scheduled/manual workflow from M10.
