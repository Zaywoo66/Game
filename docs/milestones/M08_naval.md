# M8 Naval
Docs: ARCHITECTURE.md, MAP_PIPELINE.md

Progress: sea graph and strict v3 port validation are implemented. Fleets have
deterministic movement, transport, battles, blockades, and monthly at-sea
attrition (`ceil(ships / 100)`). A stationary fleet is sheltered when its sea
zone touches an owned port or a port belonging to its side in the same war;
moving fleets always count as at sea. A sunken fleet also loses its embarked
army, and insufficient remaining capacity loses the whole army. Original
instanced low-poly ship models are implemented in the map view. Shipbuilding orders cost 5 treasury units (5,000 raw Fixed), take 30 daily ticks, and are stored in saves/LAN snapshots; capture of the selected port cancels the order without refund. The country military panel exposes construction at owned ports. Complete AI L3 remains open.
The current 1440 scenario owns no coastal port for France, so its starting
fleet remains a M11 ownership/content gap rather than an invented harbor.

## Tasks
- Sea zones graph, ports linking coast and sea zones, straits as special edges
- Fleets, transports, embark/disembark, naval battles, blockade, attrition
- Ship construction at owned ports, validated through host commands
- Ship models (instanced low-poly)

Monthly attrition is resolved after that day's fleet movement and battles on
the first Gregorian day of each month. One percent of current ships is lost,
rounded up. There are no losses while sheltered in port; no extra port/sea
state is stored.

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
