# Western Europe, 1440

The M3 scenario starts on January 1, 1440. It assigns 24 land provinces to
England, Scotland, France, Castile, Portugal and Aragon. The other 18 land
provinces are neutral. England is the starting human country in player slot
1. The five other countries use the balanced or mercantile AI profile.
Emptying the scenario's `human_slots` object creates an all-AI start.

This is a coarse playable reconstruction of the M2 map. Province polygons
do not reproduce political boundaries, and the six playable countries do
not include every polity in the slice. The English Normandy assignment is
a coarse approximation. The rest of the French and Iberian allocations
leave smaller, separate or contested territories neutral where possible.

The ruler names and lifetimes draw on historical reference works. Where
only a year was used, the scenario places the birthday on January 1.
Humphrey of Gloucester serves as England's illustrative starting heir.
Edward of York and James of Scotland are future characters, so they are
ineligible at the start. The succession model can diverge from recorded
history. Traits, ADM/DIP/MIL, fiscal balances, AI profile weights and rules
are original gameplay values, with no claim to historical measurements.

Historical references:

- [Henry VI, Royal Household](https://www.royal.uk/henry-vi) and
  [Humphrey of Gloucester, British Library catalogue](https://searcharchives.bl.uk/catalog/040-003481581)
- [James II, Royal Household](https://www.royal.uk/james-ii-r-1437-1460) and
  [James III, Royal Household](https://www.royal.uk/james-iii-r-1460-1488)
- [Charles VII, Bibliothèque nationale de France](https://catalogue.bnf.fr/ark:/12148/cb133203637.public)
- [Iberian royal dynasties, Real Academia de la Historia](https://www.rah.es/dinastias-reales/) and
  [John II of Aragon, Historia Hispánica](https://historia-hispanica.rah.es/biografias/25004-juan-ii-de-aragon-y-de-navarra)
- [Afonso V, Bibliothèque nationale de France](https://gallica.bnf.fr/accueil/fr/html/circulations-luso-mediterraneennes-a-la-renaissance)

Validate the scenario and all of its references using:

```powershell
cargo run --locked -p strategy-data --bin lint-scenario -- data/scenarios/1440.json data/map/western_europe/provinces.json
```
