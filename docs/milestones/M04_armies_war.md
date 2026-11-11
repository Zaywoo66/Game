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
