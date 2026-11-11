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
