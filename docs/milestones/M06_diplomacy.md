# M6 Diplomacy
Docs: SIM_RULES.md, DATA_FORMATS.md

## Tasks
- Opinion model; treaties: alliance, guarantee, vassalage, royal marriage; truces
- Proposal queue between players; AI response by opinion formula
- Diplomacy window: country list, country card, actions with disabled-reason tooltips

## Acceptance
- Every action has validation + typed reason + unit test
- Player-to-player proposal accepted/rejected end-to-end over the network

## AI slice (required)
Docs: AI.md
- **AI L2**: AI answers proposals using the same acceptance formula shown to humans, proposes alliances/guarantees/marriages/truces, reacts to threats and broken treaties
- Acceptance formula returns a list of factors (used by UI tooltip and AI debug panel)
- Acceptance add-on: scenario tests — AI accepts a beneficial alliance, rejects while hostile, honors truce
