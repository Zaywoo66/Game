# MAP_PIPELINE

## Source data (public domain / open licenses; log each in ASSETS.md)
- Natural Earth 5.1.2 provides 1:50m land, lakes and rivers, plus 1:10m populated places for city seeds. NOAA ETOPO1 provides elevation and bathymetry.
- M2 uses the territorial 1440 polity layer from Seshat Cliopatria 0.2.1 (CC BY 4.0). Only `POLITY` features are used; relational overlays are not territorial polygons. A reproducible project correction splits the source's obsolete Bavaria-Straubing components: the eastern pieces remain Bavaria, while the western pieces become `Burgundian Netherlands`. Philip the Good acquired Holland and Zeeland by 1433 ([BMGN historical study](https://bmgn-lchr.nl/article/download/URN%253ANBN%253ANL%253AUI%253A10-1-102327/580/657), [Dutch National Archives record](https://www.nationaalarchief.nl/onderzoeken/archief/3.03.01.01/beschrijving/appendices)); Hainaut passed to Burgundy in the same period ([Cambridge University Press](https://assets.cambridge.org/97805218/43522/excerpt/9780521843522_excerpt.pdf)). The correction is based on source geometry components west/east of longitude 6, is intentionally coarse, and has a regression test.

## Tools
- Python (`rasterio`, `shapely`, `scipy`, `numpy`, `Pillow`) in `/tools/map`. QGIS for manual inspection/edits.
- Install the pinned dependencies with `python -m pip install -r tools/map/requirements.txt`.
- Generate the city-seeded slice offline from the checked-in source subsets with `python tools/map/build.py --region western_europe`. `tools/map/fetch_inputs.py` reproduces the clipped source inputs. Run the same build with `--update-scenario` to refresh 1440 ownership and forts.
- Validate the map with `python tools/map/validate.py --region western_europe` and validate its schema and hash with the Rust `lint-provinces` CLI. CI also runs the browser picking and owner lookup tests.

## Outputs (`/data/map/<region>/`)
| File | Meaning |
|------|---------|
| provinces.png | RGB24 ID, `(red << 16) | (green << 8) | blue`, with nearest sampling and no antialiasing |
| provinces.json | Schema v3 graph, projection and area keys. See DATA_FORMATS.md. |
| areas.yaml, regions.yaml | Area and region tables with historical provenance where available. |
| names.en.json | English fallback names for provinces, areas and regions. |
| polities_1440.json | Cliopatria 1440 best-overlap evidence for every land province, pinned to the map hash; unmatched and coarse groups remain explicit. |
| heightmap.png | 16-bit grayscale elevation in metres, decoded as sample minus 32768 |
| terrain.png | 8-bit terrain index: 0 sea, 1 plains, 2 hills, 3 mountains, 4 lake |
| rivers.png | 8-bit mask: 0 background, 255 river |
| water mask | derived from sea provinces |

`polities_1440.json` is generated offline from the checked-in clipped
Cliopatria source after the documented Bavaria-to-Burgundian-Netherlands
correction. `tools/map/assign_1440_ownership.py` seeds scenario ownership from
named polygon matches covering at least 30% of a province's pixels, only for
country tags already present in the scenario and only when the province has no
owner. Pass `--fill-unowned` to complete the 1440 campaign using the documented
nearest-owner fallback after named matches. The default majority threshold is 50%; an explicit 30% edge-coverage
pass fills additional named matches for already represented countries. Its
30–50% assignments are lower-confidence approximations, traceable in
`polities_1440.json`. For the full-Europe 1440 scenario, coarse groups, unmatched cells, and
unrepresented named polities are assigned using the owner-approved nearest-owner
fallback. HRE coarse cells are restricted to represented HRE member states. The
result completes playable ownership but is a gameplay approximation, not an
administrative survey; historical rendering still shows source evidence separately. The source release, license,
processed-layer SHA-256 and correction method are recorded in the sidecar; the
Python map validator checks provenance and land coverage.

The optional `tools/map/audit_aoc2_1440.py` comparison reads an installed
Age of Civilizations II 1440 scenario locally and writes a temporary CSV.
It does not copy the proprietary game geometry or scenario into this repo.
`tools/map/add_1440_minor_polities.py` uses reviewed, high-agreement city and
province samples to add separate playable minor states; the permanent
country records and owner overrides are project content. The local game is a
political reference, not historical proof. Source-checked named-city
corrections and still-uncertain areas are listed in M11_content.md. The
fallback preserves an existing represented HRE member owner and every
explicitly reviewed override on rerun, while assigning still-unowned coarse
HRE cells only to eligible member states.

The owner-approved steppe policy is pinned to the full-Europe map identity. It interprets the screenshot label “Golden Horde” as a legacy region label and assigns one Great Horde country, with a separate Nogai Horde, Uzbek Khanate (extended through Tura), and Crimean Khanate. The longitude bands are gameplay approximations; see `data/map/europe/ownership_1440.json`. These assignments change scenario owners, not the historical evidence layer.

Preview ownership changes with `python tools/map/assign_1440_ownership.py`;
`--apply` writes the sorted province lists to the 1440 scenario. Tests cover
the majority threshold, aliases, unknown groups, and existing-owner safety.

Preview neighbor-aware country colors with `python tools/map/color_1440_countries.py`;
`--apply` writes a muted palette to all scenario country files, maximizing
color distance between countries that share a land border.

The generator writes symmetric movement costs in the map metadata using the
region's `movement_costs` in `overrides.yaml`. Changing these costs changes
the metadata hash, so existing saves are rejected even if the schema version
remains 3. The four rasters are unaffected.

## Province generation
The Western Europe crop uses 32 pixels per degree on both axes, with an
800 × 744 raster and global origin offset `[5344, 984]`. Province IDs
use the reserved European range. Settlement seeds and water seeds are
processed independently of starting ownership.

For the 1440 scenario, assign the largest historical polygon overlap,
apply the sourced overrides and explicitly mark land outside the six
playable countries as unowned. Historical area records refer to cited
jurisdictions. Cell footprints approximate membership and do not trace
historical administrative boundaries. Where local evidence is missing,
an unclassified area preserves the gap.

Island settlements remain provinces. Ports retain contact with the sea.
The dense slice adds no over-water land connections. Sea access is
scheduled for M8.

## Europe alpha extent

`python tools/map/fetch_europe.py` fetches checksum-pinned Natural Earth v5.1.2
and Cliopatria v0.2.1 subsets plus a value-pinned NOAA ETOPO1 subset. The
clipped inputs are checked in. `python tools/map/build_europe.py --region europe`
builds `data/map/europe/` offline. The 2592 × 1152 raster covers longitudes
−13 to 68 and latitudes 36 to 72 at 32 pixels per degree. Its checked-in
version-5 baseline preserves the former 2336 × 1152 raster pixel for pixel;
new cells are appended eastward, including a Tura/modern Tyumen capital cell.
The western 800 × 744 raster remains embedded pixel for pixel at x=0..799,
y=408..1151. Existing province IDs and keys remain stable; the extension
stays below the reserved Europe ID ceiling of 2000. The full map is version 6
and has its own hash because its graph and extent differ from earlier maps.

New land cells use ranked Natural Earth settlements and sparse coordinate
fallbacks. Cliopatria's largest polygon overlap is recorded for every land
province with `polygon`, `coarse_group`, or `unmatched` resolution. These are
reference colors and source evidence, not scenario ownership. The packaged
host and default UI both load the Europe graph, so picking, commands, path
queries, LAN identity and saves share its map version and hash. The scenario assigns every land province. Cells without an existing-country
polygon match use the documented nearest-owner fallback, while the default
Historical 1440 view continues to show source evidence. Use
`?map=western_europe` only to inspect the legacy slice without a live host.

The current full-Europe sidecar classifies 1028 land provinces as `polygon`,
213 as `coarse_group` (74 HRE minor-state cells and 139 Scandinavian cells
from a source group labeled `Kalmar Union`), and 43 as `unmatched`. The source
group is not treated as one sovereign state. The owner-approved gameplay
partition assigns those 139 cells among independent Denmark, Norway, and
Sweden using explicit southern/island overrides and a documented approximate
Norway–Sweden line. This is a campaign ownership approximation, not a border
claim derived from the coarse source layer. Iceland is outside the current
raster extent (western edge −13°) and is not represented. The 74 coarse HRE cells are assigned to the nearest represented HRE member
by map-graph distance; the HRE remains a confederation, never a single owner.
Microstates absent from the playable roster therefore use an explicitly marked
gameplay approximation rather than remaining neutral. The map geometry is from
Cliopatria and Natural Earth, not extracted Europa Universalis IV files; game
screenshots may be used as visual references, not redistributed map data. The
owner has approved noncommercial use of the [ETH Zurich HRE vector
dataset](https://www.research-collection.ethz.ch/entities/researchdata/2f7da3dd-d3ef-42f9-9a46-6504511ca522)
(CC BY-NC-SA 4.0). Its c.1400 generated territories are an approximate
reference, not verified 1440 sovereignty. The derived
`tools/map/inputs/hre_eth/hre_overlay_1440.json` crosswalk maps only coarse
cells with strong raster overlap and leaves ambiguous cells unassigned; it is
not campaign ownership or authoritative 1440 evidence. The dataset and
derivatives are restricted to noncommercial use and retain CC BY-NC-SA 4.0.
The public-domain [Droysen 1400 map](https://commons.wikimedia.org/wiki/File:Heiliges_R%C3%B6misches_Reich_1400.png)
remains a visual reference, not sufficient evidence by itself to assign
individual province cells.

## Rendering (`/ui/src/map`, three.js WebGL2)
- Terrain mesh from heightmap (chunked), custom shaders: terrain blend, hillshade, rivers, depth-colored water, province/area/country borders, owner color via lookup texture (`provinceId -> ownerColor`).
- An orthographic camera clamps zoom, tilt and pan. A low ocean skirt and a short edge fade cover the finite raster edge in wide or tilted views; land outside the regional source extent is not inferred.
- Picking: CPU copy of provinces raster, read pixel under cursor -> province id.
- Units: `InstancedMesh`, < 500 triangles each, shared texture palette. Sea units same approach.
- Camera: pan, zoom, tilt limits. Map wraps are NOT supported in beta.
- Update ownership by writing bytes to the lookup texture, never regenerate the map texture.
- Capture four repeatable 1920 × 1080 browser screenshots with `cd ui && npm run capture:map`; output is `docs/screenshots/m5b4/`. The capture uses Chromium's software WebGL renderer and is visual review evidence, not a hardware FPS result.
- M5b.5 adds Game political, Terrain and Historical 1440 display modes. Without a game snapshot, the map starts in Historical 1440 with a visible reference-only legend. Once an active game snapshot arrives, it defaults to Game political unless the player manually chose a mode. Game political ownership colors come only from host `GameSnapshot.owners`; the historical view uses a separate lookup from `polities_1440.json` after map-version/hash validation. Named polygon matches receive polity colors and labels. Coarse and unmatched cells get deterministic placeholder tints; unresolved source precision remains visible in the province inspector. Coarse Scandinavian cells receive visual-only regional labels for Denmark, Norway, Sweden and Iceland, with separate tints rather than one unified Kalmar fill. These tints fill the full political picture but do not claim sovereignty. The presentation overlay uses `names.en.json` for province names; Russian localization has not been authored. Sea-cell outlines are subdued in ordinary modes while raster picking remains active.
- The Game political view uses named 1440 polygon colors as a translucent visual fallback for provinces without a campaign owner; host-owned provinces remain opaque and authoritative. Coarse/unmatched fallback cells also receive muted source fills. The province inspector reports source attribution separately from campaign ownership. HRE campaign cells belong to represented member states by nearest graph distance; no HRE umbrella state is created. The evidence is from Cliopatria, not extracted Europa Universalis IV data; EU4 and Bloody Europe screenshots are visual references only.
- Political boundaries receive a deterministic, smooth coordinate warp of at most six raster pixels in the shader to soften the raster-Voronoi look. Pointer picking uses the same transform; province IDs, adjacency, and saved-map identity stay unchanged by this presentation effect.
- Named polity colors use a deterministic 108-color muted atlas palette assigned by perceptual contrast against neighboring polities. Unresolved cells receive deterministic, restrained tints varied within broad source-group color families; these are map-fill placeholders, never owners. Country borders render wider and darker than internal province edges; internal lines are thinner and lighter at continental zoom. Political fills retain subtle terrain relief, and water uses subdued blue bathymetry with a shallow coastal shelf and restrained wave motion. Hatching remains a low-key cue for uncertain source matches, not a gray replacement fill.
- Capture four named Historical/Terrain views, one hover/selection view, and visual-only army/fleet marker demonstrations with `cd ui && npm run capture:map-modes`; output is `docs/screenshots/m5b5/` (Chromium software renderer, not hardware FPS evidence).

## Budgets
Province raster for beta region <= ~4096x2048. To measure the 1080p viewport, run the built Windows executable with `--map-benchmark`. It opens fullscreen and displays the actual drawing-buffer size and frame timings. Record measured RAM and FPS in DECISIONS.md with the GPU and method.
For repeatable RAM sampling, keep the native benchmark window open and sample its private working set from a separate PowerShell terminal with `Get-Process strategy-app | Select-Object ProcessName,Id,@{Name='PrivateMiB';Expression={[math]::Round($_.PrivateMemorySize64 / 1MB, 1)}}`. Record the FPS text after its 2-second warmup and 10-second measurement, plus this memory sample and the renderer string shown in the app. This native measurement is still required for M5b acceptance.
