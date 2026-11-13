# ETH HRE territory reference snapshot

This folder contains the `territories_auto_semi` shapefile from Ramona Roller's
2021 ETH Zurich dataset, *Static spatial data on territories of the Holy Roman
Empire*, DOI [10.3929/ethz-b-000472585](https://doi.org/10.3929/ethz-b-000472585).
The dataset and this derivative overlay are licensed CC BY-NC-SA 4.0; they may
not be used commercially and any adaptation must retain the same license.
See the ETH [dataset record](https://www.research-collection.ethz.ch/entities/researchdata/2f7da3dd-d3ef-42f9-9a46-6504511ca522)
for license and creator details.

The source is a static, automatically generated approximation of HRE
territories around 1400, compiled from a modern HRE map and Wikipedia pages.
It is not a verified political-boundary snapshot for 1440. Accordingly,
`hre_overlay_1440.json` is a candidate crosswalk for review, not game ownership
or a historical claim. It leaves a coarse province unassigned unless one
source territory covers at least 50% of its raster pixels and leads the runner
up by at least 15 percentage points. Uncovered and ambiguous cells remain null.

Reproduce from the repository root after installing the pinned map tools:

```powershell
python -m pip install -r tools/map/requirements.txt
python tools/map/overlay_hre_territories.py
```

The script reads the existing Europe map and its 1440 source-resolution
sidecar; it does not modify provinces, map topology, or country/scenario data.
