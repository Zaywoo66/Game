# M2 map generator

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
- Cliopatria v0.1.3, Ed Chalstrey, James Bennett and the Seshat Global History
  Databank. CC BY 4.0. Polity geometries active in 1440 were selected, clipped
  and simplified. Their boundaries bias the land Voronoi assignment. The
  output is a prototype gameplay map and does not assert exact historical
  ownership. [Source](https://doi.org/10.5281/zenodo.14714684)
  · [Release](https://github.com/Seshat-Global-History-Databank/cliopatria/tree/v0.1.3)
  · [License](https://creativecommons.org/licenses/by/4.0/)
- ETOPO1 1 Arc-Minute Global Relief Model, NOAA NGDC (Amante and Eakins, 2009).
  Public domain. The NOAA AOML ERDDAP service supplies numeric ice-surface
  altitude in meters on a two-sample stride. The vendored arrays preserve
  coordinates and signed elevations, and the generator bilinearly samples
  them onto the map. [Source](https://doi.org/10.7289/V5C8276M)
  · [Subset service](https://cwcgom.aoml.noaa.gov/erddap/info/etopo180/index.html)

## Raster and topology

The 768 by 1024 equirectangular slice covers longitudes -13 to 12 and
latitudes 36 to 59.25. Pixel centers determine sampling. The map has 42 land
provinces, six sea provinces and one lake province.

`overrides.json` supplies named seeds, two named strait crossings and the
land provinces that get a port. Land cells use raster Voronoi distances
restricted by the coastline, with crossing penalties for rivers and 1440
polity boundaries. Small land fragments without a seed are assigned intact
to the nearest seed. Sea and lake labels use nearest seeds inside their masks.

Cardinal pixel contact generates adjacency. At least three river pixels and
20% river coverage on a shared land boundary give a river edge. Dover Strait
and the North Channel are explicit, checked crossings. Each selected port
attaches to the sea province with the longest shared coastline.

Province pixels encode RGB24 IDs as `(R << 16) | (G << 8) | B`. The 16-bit
grayscale relief uses `sample - 32768` meters. The 8-bit grayscale terrain
indices are 0 sea, 1 plains, 2 hills (at least 350 m), 3 mountains (at least
1,400 m), and 4 lake. Rivers use 0 or 255 on land. All PNGs are noninterlaced.
