# Western Europe map generator

## Full Europe alpha reference map

Run `python tools/map/fetch_europe.py` only when refreshing the pinned source
subsets. Build the checked-in source data offline with
`python tools/map/build_europe.py --region europe`, then run
`python tools/map/assign_1440_ownership.py --threshold 0.3 --fill-unowned --apply` and
`python tools/map/color_1440_countries.py --apply` to refresh scenario owners
and distinct neighbor-aware country colors, then run
`python -B -m unittest tools/map/tests/test_europe.py` and
`cargo run --locked -p strategy-data --bin lint-provinces -- data/map/europe/provinces.json`.
The default UI opens the colored Historical 1440 view. The packaged host
loads the same Europe graph. The 1440 scenario assigns every land province; low-confidence and
source-unmatched cells use the documented nearest-owner approximation. The Europe map preserves its version-5 raster and province
IDs while appending the eastern extension, and still preserves western pixels
and IDs at offset `(0, 408)`. The build uses the pinned version-5 baseline
archive so repeated builds produce the same extension.

Install the pinned dependencies using Python 3.12, then build and verify the
offline Western Europe slice:

```powershell
python -m pip install -r tools/map/requirements.txt
python tools/map/build.py --region western_europe
python tools/map/validate.py --region western_europe
python -B -m unittest discover -s tools/map/tests
cargo run --locked -p strategy-data --bin lint-provinces -- data/map/western_europe/provinces.json
```

The input and output directories are `tools/map/inputs/western_europe/` and
`data/map/western_europe/`. The build validates staged files before replacing
the outputs. Change the input or override files and rebuild to update the map.
Use `--update-scenario` to also refresh 1440 ownership and the existing forts
and capitals in `data/scenarios/1440.json` and `data/countries/`.

## Source snapshots

`fetch_inputs.py` regenerates the vendored source subsets, using SHA-256 pins
for each upstream source. It stores large downloads in the system temporary
directory by default. The map build itself requires no network access.

- Natural Earth 1:50m physical vectors, release v5.1.2. Tom Patterson,
  Nathaniel Vaughn Kelso and contributors. Public domain. Land polygons,
  Lake Geneva and river centerlines are clipped to the view and simplified.
  [Dataset](https://www.naturalearthdata.com/downloads/50m-physical-vectors/)
  · [Release](https://github.com/nvkelso/natural-earth-vector/tree/v5.1.2)
  · [Terms](https://www.naturalearthdata.com/about/terms-of-use/)
- Natural Earth 1:10m Populated Places, release v5.1.2. Tom Patterson,
  Nathaniel Vaughn Kelso and contributors. Public domain. The vendored slice
  contains settlement names, ranking, population, and point coordinates.
  [Dataset](https://www.naturalearthdata.com/downloads/10m-cultural-vectors/10m-populated-places/)
  · [Terms](https://www.naturalearthdata.com/about/terms-of-use/)
- Cliopatria v0.2.1, Ed Chalstrey, James Bennett and the Seshat Global History
  Databank. CC BY 4.0. Territorial polity geometries active in 1440 are
  selected, clipped and simplified. Relation overlays are excluded, and the
  documented 1433 transfer correction relabels western Bavaria-Straubing
  components as `Burgundian Netherlands`. The output is a prototype gameplay map
  and does not assert exact historical ownership.
  [Source](https://doi.org/10.5281/zenodo.14714684)
  · [Release](https://github.com/Seshat-Global-History-Databank/cliopatria/tree/v0.2.1)
  · [License](https://creativecommons.org/licenses/by/4.0/)
- ETOPO1 1 Arc-Minute Global Relief Model, NOAA NGDC (Amante and Eakins, 2009).
  Public domain. The NOAA AOML ERDDAP service supplies numeric ice-surface
  altitude in meters on a two-sample stride. The vendored arrays preserve
  coordinates and signed elevations, and the generator bilinearly samples
  them onto the map. [Source](https://doi.org/10.7289/V5C8276M)
  · [Subset service](https://cwcgom.aoml.noaa.gov/erddap/info/etopo180/index.html)

## Raster and topology

The 800 by 744 equirectangular slice covers longitudes -13 to 12 and
latitudes 36 to 59.25. It uses 32 pixels per degree with a northwest
crop origin of [5344, 984] in the global grid. The map has 362 land
provinces, 66 sea zones and one lake province.

Ranked settlement seeds snap within five land pixels and keep a minimum
spacing of three pixels. Land cells use raster Voronoi distances restricted
by coastline, with crossing penalties for rivers and 1440 polity boundaries.
Land fragments without a seed attach intact to their nearest land seed.
Water zones use sparse open-water grid seeds; lakes have their own mask.

`overrides.yaml` supplies the 12 existing port locations, reassigned to
coastal settlement cells. Cardinal pixel contact
generates adjacency. At least three river pixels and 20% river coverage on
a shared land boundary give a river edge. Dover Strait and the North Channel remain open water with maritime access
deferred to M8. Each selected port attaches to the sea province with the
longest shared coastline. Region `movement_costs` set
positive whole movement ticks and remain symmetric. Normal land edges take
three game days and river edges take five at base movement speed. Map schema v3 hashes
the projection and canonical province records, including their hierarchy keys.

Starting ownership uses the highest pixel overlap by Cliopatria polity name
and applies sourced exceptions in `overrides.yaml`. Provinces outside the six
the western-slice baseline still marks land outside its six playable countries as unowned; the full-Europe 1440 scenario applies the separate ownership fallback. `administrative_areas.json`
maps selected settlement cells to sourced historical areas. Unclassified
areas mark gaps in coverage. Area boundaries follow gameplay cells.
`areas.yaml` and `regions.yaml` describe the generated hierarchy and
`names.en.json` provides English display names.

`polities_1440.json` records the largest Cliopatria 1440 polygon overlap for
all land cells, with the checked-in source SHA-256 and map identity. It keeps
Irish cells with no source polygon as `unmatched` and broad polity labels such
as `Holy Roman Empire Minor States` as `coarse_group`. This evidence does not
assign scenario ownership; review local records before converting a match to
game content.

The dense ownership raises the levy estimates of neighboring countries toward
the same cap. `data/ai/profiles/balanced.json` therefore uses a war threshold
of 0 rather than 250. The 10-year military autoplay and 10,000-tick LAN replay
both still require an AI-negotiated treaty; revisit this balance with the
movement and performance measurements in 5b.3.

Province pixels encode RGB24 IDs as `(R << 16) | (G << 8) | B`. The 16-bit
grayscale relief uses `sample - 32768` meters. The 8-bit grayscale terrain
indices are 0 sea, 1 plains, 2 hills (at least 350 m), 3 mountains (at least
1,400 m), and 4 lake. Rivers use 0 or 255 on land. All PNGs are noninterlaced.
