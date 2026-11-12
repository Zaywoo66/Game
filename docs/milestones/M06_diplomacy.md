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
is part of the approved M6 rule. The country-list diplomacy window projects
effective opinions, active alliances, alliance proposals with expiry, and
guarantee/vassalage controls. It sends alliance offers, responses, and breaks
through host-validated commands; the UI changes only on an authoritative
snapshot or update. Browser coverage checks this command/update path. Two-PC
LAN acceptance and the remaining marriage proposal UI are still open.

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

## Vassalage slice

The would-be overlord sends a directed 30-day offer to a prospective subject.
Both countries must be at peace when the subject accepts. One subject can have
one overlord, no subject can itself be an overlord, and an overlord may have
multiple subjects. The overlord may release a subject immediately when both
are at peace. A subject controls its own economy and armies but cannot declare
an external war or sign an independent treaty. As defensive war leader, the
subject may negotiate peace in its own war.

Each month the subject pays 10% of post-modifier tax income to the overlord in
raw Fixed units, truncated toward zero. The transfer is deducted before the
subject spends and credited to the overlord in the same month. Gross income,
tribute paid, tribute received and paid expenses are visible in the budget.

When a subject is attacked, a free overlord joins its defensive side. If the
overlord is already at war or allied with the attacker, the subject fights
alone and loses 25 opinion toward the overlord; the bond remains. If the
overlord also guaranteed the subject, that guarantee ends and the combined
failure costs 25 opinion once.

A subject may declare an independence war against its overlord, targeting the
overlord's capital. Enforcing the goal needs the normal minimum war score and
attacker-side occupation of that capital. It ends the bond without ceding the
capital. White peace retains the bond. Only the two war leaders negotiate.

AI replies through validated commands. It accepts a legal vassal offer only
when effective opinion meets `vassal_min_opinion` and the overlord has strictly
more active soldiers; the current profiles set the threshold to 20 points.
Proactive AI vassal proposals and the wider diplomacy window remain open.
