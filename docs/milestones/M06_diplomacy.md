# M6 Diplomacy
Docs: SIM_RULES.md, DATA_FORMATS.md

## Tasks
- Opinion model; treaties: alliance, guarantee, vassalage, royal marriage; truces
- Proposal queue between players; AI response by opinion formula
- Diplomacy window: country list, country card, actions with disabled-reason tooltips

## Acceptance
- Every action has validation + typed reason + unit test
- Player-to-player proposal accepted/rejected end-to-end over the network

Royal-marriage mechanics, AI and wire commands have synthetic tests. The full
player-to-player UI acceptance remains open with the wider M6 diplomacy screen.
The 1440 content has no eligible female character or modeled prior spouse;
historical marriage content belongs to M11.

## AI slice (required)
Docs: AI.md
- **AI L2**: AI answers proposals using the same acceptance formula shown to humans, proposes alliances/guarantees/marriages, honors automatic post-war truces, and reacts to threats and broken treaties
- Acceptance formula returns a list of factors (used by UI tooltip and AI debug panel)
- Acceptance add-on: scenario tests — AI accepts a beneficial alliance, rejects while hostile, honors truce

## Post-war truce visibility slice

Peace automatically creates a truce for the original war leaders and every
pair of countries on opposing war sides. The existing war validation blocks
redeclaration until `truce_ticks` days have elapsed. The Tauri snapshot and
updates project only active pairs, remaining game days and the calculated end
date. The country card shows each pair; the military panel disables a war
declaration against a country under truce and states the expiry reason. AI war
commands already pass the same validation. No separate peacetime truce offer
is part of the approved M6 rule. The full diplomacy window and player-to-player
proposal acceptance remain open.

## Defensive guarantee slice

A guarantee is a unilateral, indefinite commitment from one country to another.
It can be created only while both are at peace. Multiple guarantors may protect
one country, and a country may guarantee an ally. The guarantor can revoke in
any season with no cooldown; the protected country's opinion of the guarantor
falls by 10 points. An attacker must revoke its own guarantee before declaring
war on the protected country.

When a protected country is attacked, each free guarantor joins the defender
side of that war. If a guarantor is already in another war or is allied with
the attacker, its guarantee instead ends and the protected country's opinion
of it falls by 25 points. The alliance with the attacker stays in force.
Original war leaders negotiate peace, which ends the war for all participants
and gives every pair of former opponents the same truce. AI issues guarantees
through validated commands on its 31-tick diplomacy cadence for a non-allied
country it views favorably when both face a common stronger neighbor. Typed
Tauri commands and snapshot fields are available; visible diplomacy controls
and the M6 network proposal acceptance remain open.
