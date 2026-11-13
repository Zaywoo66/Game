# Europe, 1440 (alpha scenario)

The current scenario starts on January 1, 1440 on Europe map version 6. The
map contains 1,466 provinces, including 1,284 land provinces. Forty-five countries
and 56 characters are loaded; England is assigned to human slot 1 and the other
countries are AI-controlled.

All forty-five countries together own all 1,284 land provinces; no land province
is neutral. Ownership begins with named 1440 source polygons, then applies the
owner-approved steppe and Scandinavian rules and a deterministic nearest-owner
fallback for unresolved cells. Coarse HRE cells go only to represented HRE
members, preserving the Empire as a confederation rather than a unified country.
The 145 source cells labeled `Novgorod Republic` belong to separate country `nov`.
Its posadnik Esif Grigorievich uses birth year 1400 as an explicit gameplay
estimate because the source does not provide a birth date. Other microstates
absent from the roster and source gaps are gameplay approximations; see
`data/map/europe/ownership_1440.json` and
`docs/milestones/M11_content.md` for method and limitations.

The scenario begins with nine fort levels across seven provinces, no armies or
wars, and five two-ship fleets assigned to Aragon, Castile, England, Portugal
and Scotland. The other forty countries have no starting fleet in this
snapshot: Archduchy of Austria, Bosnia, Brandenburg, Brittany, Burgundy, House of Bourbon,
Kingdom of Denmark, Republic of Florence, France, Georgia, Granada, Great Horde,
Hafsid, Hungary, Kazan, Lithuania, Luxembourg, Milan, Moscow, Naples, Nevers,
Nogai Horde, Kingdom of Norway, Ottomans, Papal States, Poland, Qara Qoyunlu,
Savoy, Kingdom of Sweden, Teutonic Order, Timurid Empire, Trebizond, Moldavia,
Uzbek Khanate, Crimean Khanate, Venice and Karaman.
All forty-five scenario countries have registered original simplified SVG banners.
Existing generic event
and idea systems remain available, while country-specific historical events
and unique national modifiers are deferred until post-alpha. Some European microstates are represented by the documented nearest-owner
approximation rather than as separate playable countries.

Validate the scenario and all of its references using:

```powershell
cargo run --locked -p strategy-data --bin lint-scenario -- data/scenarios/1440.json data/map/europe/provinces.json
```
