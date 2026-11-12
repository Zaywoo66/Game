# M8 Naval
Docs: ARCHITECTURE.md, MAP_PIPELINE.md

Progress: sea graph and strict v3 port validation are implemented. The first
fleet/transport slice is in progress. Deterministic naval battles and automatic
port blockades are implemented; original instanced low-poly ship models are
implemented in the map view. Attrition, construction and complete AI L3 remain
open.
The current 1440 scenario owns no coastal port for France, so its starting
fleet remains a M11 ownership/content gap rather than an invented harbor.

## Tasks
- Sea zones graph, ports linking coast and sea zones, straits as special edges
- Fleets, transports, embark/disembark, naval battles, blockade, attrition
- Ship models (instanced low-poly)

## Acceptance
- Scripted amphibious invasion test passes; blockade closes port income

During a war, an enemy fleet in any adjacent sea zone blockades a port when
no fleet on the port owner's war side is in that zone. The blockade ends as
soon as the enemy fleet leaves or the war ends. On the first day of a month,
the blocked port province contributes no taxable development to its owner's
income; other provinces remain taxable. This is also shown in the budget
forecast and province inspector. No separate blockade command is needed.

## AI slice (required)
Docs: AI.md
- **AI L3 (naval)**: build/maintain fleets, escort transports, run amphibious invasions, blockade, avoid attrition
- Acceptance add-on: AI performs at least one amphibious landing in a scripted scenario; no fleets stuck forever
