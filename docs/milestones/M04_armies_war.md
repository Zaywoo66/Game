# M4 Armies and land war
Docs: SIM_RULES.md, MAP_PIPELINE.md

## Tasks
- Army units, raise/merge/split; A* pathfinding over province graph with edge costs
- Movement with progress 0..1 per edge; UI interpolation of positions
- Battles (same-province hostile armies), morale, casualties, retreat
- Sieges by fort level, occupation, war declaration, war goals, peace treaty
- Supply/attrition (simple); first instanced 3D unit models

## Acceptance
- Scripted war between two countries ends in a peace treaty with territory change
- Tests: pathfinding, battle resolution, siege progress, occupation -> ownership

## AI slice (required)
Docs: AI.md
- **AI L1**: raise armies, merge/split, move to targets, defend capital, besiege forts, retreat when losing, choose war targets among weaker neighbors, accept/propose peace by war score
- Debug panel: last AI decisions with reasons and scores
- AI smoke test in CI (10 game years, 3 seeds, no panic/invalid state)
- Acceptance add-on: AI-vs-AI war between two countries reaches a peace treaty without human input; AI never issues invalid commands

## Land-war slice

Army orders run on traversable land edges through the country's own land
and land owned by its active enemy. The graph stores daily movement costs.
The map interpolates the army between province centers from the host's
reported progress. A shared low-poly infantry model represents armies.

Each country can participate in one war at a time. Declarations require a
land frontier and a reachable non-capital war goal. Combat compares soldiers,
morale and modifiers. The defending party wins tied strength. Casualties,
retreat, supply and fort timing follow the scenario rules. The losing force
retreats to eligible friendly land or disperses if cut off.

Occupation changes control during war. A treaty can end in white peace or
cede the occupied goal when the attacker's score meets the scenario threshold.
Both parties must agree. The treaty updates territorial ownership and the
scenario truce blocks an immediate rematch.

The scripted simulation test covers battle, retreat, a four-day fort siege,
occupation and territorial peace. The workspace AI gate runs three seeded
ten-year simulations and checks their invariant and error reports each day.
Each run reaches a treaty with a cession, and the replay from a year-five
save produces the same final hash. DECISIONS.md records the longer soak runs.
