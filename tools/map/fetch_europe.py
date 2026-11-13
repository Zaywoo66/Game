"""Fetch pinned public source layers for the geographic Europe extension."""

import argparse
from pathlib import Path
import tempfile
from hashlib import sha256

import numpy as np
from scipy.io import netcdf_file

from shapely.geometry import box

import fetch_inputs as source


BOUNDS = (-13.0, 36.0, 68.0, 72.0)
INPUTS = Path(__file__).resolve().parent / "inputs" / "europe"
RELIEF = ("https://cwcgom.aoml.noaa.gov/erddap/griddap/etopo180.nc"
          "?altitude%5B(36):2:(72)%5D%5B(-13):2:(68)%5D")
RELIEF_DATA_HASH = "7ca249d6a568fd35a3dd6155c18d75d1f98e1940636553f4a722a043da9bf281"


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, default=Path(tempfile.gettempdir()) / "strategy-map-m2-inputs")
    args = parser.parse_args()
    args.cache.mkdir(parents=True, exist_ok=True)
    INPUTS.mkdir(parents=True, exist_ok=True)
    source.BOUNDS = BOUNDS
    source.CLIP = box(*BOUNDS)
    source.INPUTS = INPUTS
    source.physical_layers(args.cache)
    lakes = source.read_json(args.cache / "lakes.geojson")["features"]
    source.write_layer(INPUTS / "lakes.geojson", [
        (geometry, {}) for feature in lakes
        for geometry in source.clipped(feature, ("Polygon", "MultiPolygon"))
    ])
    source.populated_places_layer(args.cache)
    source.historical_layer(args.cache)
    relief = source.download(args.cache, "etopo1-europe-68e.nc", RELIEF)
    with netcdf_file(relief, mmap=False) as dataset:
        lat = dataset.variables["latitude"].data.astype("<f8")
        lon = dataset.variables["longitude"].data.astype("<f8")
        altitude = dataset.variables["altitude"].data.astype("<i2")
    digest = sha256(lat.tobytes() + lon.tobytes() + altitude.tobytes()).hexdigest()
    if RELIEF_DATA_HASH and digest != RELIEF_DATA_HASH:
        raise ValueError(f"Europe ETOPO1 source checksum mismatch: {digest}")
    np.savez_compressed(INPUTS / "elevation.npz", latitude=lat, longitude=lon, altitude=altitude)
    print(f"Regenerated pinned Europe source subsets in {INPUTS}")


if __name__ == "__main__":
    main()
