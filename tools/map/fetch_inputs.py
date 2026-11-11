"""Rebuild the vendored subsets from the recorded source snapshots."""

import argparse
from hashlib import file_digest, sha256
from pathlib import Path
import shutil
import tempfile
from urllib.request import Request, urlopen
from zipfile import ZipFile
import json

import numpy as np
from scipy.io import netcdf_file
from shapely import make_valid
from shapely.geometry import box, mapping, shape

from map_common import BOUNDS, INPUTS, json_bytes, read_json

NATURAL_EARTH = (
    "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/"
    "geojson/ne_50m_"
)
PLACES = (
    "https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/"
    "geojson/ne_10m_populated_places.geojson"
)
PLACES_HASH = "9b8e3de09048ef00dfc70357dbb9fa324493f214b5e0ae4daf1aa79a8d10116b"
HISTORY = (
    "https://raw.githubusercontent.com/Seshat-Global-History-Databank/"
    "cliopatria/v0.1.3/cliopatria.geojson.zip"
)
RELIEF = (
    "https://cwcgom.aoml.noaa.gov/erddap/griddap/etopo180.nc"
    "?altitude%5B(36):2:(59.25)%5D%5B(-13):2:(12)%5D"
)
# ERDDAP adds a timestamp to the raw NetCDF. Pin the actual coordinate and
# elevation values instead, in the dtype and array order below.
RELIEF_DATA_HASH = "4566722c96695f6cf4fb55bf7bc2bc55644f0c3195619c54878e1ceef7e58088"
PHYSICAL = {
    "land": "e874b27a51d146452be360cafb3cc50c86001074a67d534113e6534682f9826b",
    "lakes": "d350b75978b26fe839b797c2c529b2fb8f47fb3983c03f4964e36d5df9378a52",
    "rivers_lake_centerlines": "f286e0ce978fde999ca2d7a78c764be08542e19b63cded52b05c12d5173ccc51",
}
HISTORY_HASH = "a6417c73f16049ff7a21c75dce52dde641b659255e22ed30294b47a35b664123"
CLIP = box(*BOUNDS)


def download(cache, name, url, expected=None):
    path = cache / name
    if not path.exists():
        temporary = path.with_suffix(path.suffix + ".download")
        try:
            request = Request(url, headers={"User-Agent": "Strategy-map-tools/1"})
            with urlopen(request, timeout=60) as response, temporary.open("wb") as target:
                shutil.copyfileobj(response, target)
            temporary.replace(path)
        finally:
            temporary.unlink(missing_ok=True)
    if expected is not None:
        with path.open("rb") as source:
            actual = file_digest(source, "sha256").hexdigest()
        if actual != expected:
            raise ValueError(f"{path}: source checksum mismatch: {actual}")
    return path


def clipped(feature, dimensions):
    geometry = shape(feature["geometry"])
    if not geometry.intersects(CLIP):
        return []
    geometry = make_valid(geometry).intersection(CLIP).simplify(0.005, preserve_topology=True)
    return matching_parts(geometry, dimensions)


def matching_parts(geometry, dimensions):
    if geometry.is_empty:
        return []
    if geometry.geom_type in dimensions:
        return [geometry]
    if geometry.geom_type == "GeometryCollection":
        return [part for member in geometry.geoms for part in matching_parts(member, dimensions)]
    return []


def feature_collection(items):
    return {
        "type": "FeatureCollection",
        "features": [
            {"type": "Feature", "geometry": mapping(geometry), "properties": properties}
            for geometry, properties in items
        ],
    }


def write_layer(path, items):
    if not items:
        raise ValueError(f"{path}: no features survived clipping")
    path.write_bytes(json_bytes(feature_collection(items)) + b"\n")


def physical_layers(cache):
    polygons = ("Polygon", "MultiPolygon")
    lines = ("LineString", "MultiLineString")
    for name, expected in PHYSICAL.items():
        source = download(cache, name + ".geojson", NATURAL_EARTH + name + ".geojson", expected)
        records = read_json(source)["features"]
        output_name = name
        if name == "lakes":
            records = [item for item in records if item["properties"]["name"] == "Lake Geneva"]
        if name == "rivers_lake_centerlines":
            records = [item for item in records if item["properties"]["featurecla"] == "River"]
            output_name = "rivers"
        kind = lines if output_name == "rivers" else polygons
        items = [(geometry, {}) for item in records for geometry in clipped(item, kind)]
        write_layer(INPUTS / f"{output_name}.geojson", items)


def populated_places_layer(cache):
    source = download(cache, "populated_places.geojson", PLACES, PLACES_HASH)
    places = []
    for feature in read_json(source)["features"]:
        geometry = shape(feature["geometry"])
        lon, lat = geometry.coords[0]
        west, south, east, north = BOUNDS
        if not (west <= lon < east and south < lat <= north):
            continue
        fields = feature["properties"]
        places.append((geometry, {
            "name": fields["NAME"],
            "rank": fields["SCALERANK"],
            "population": fields["POP_MAX"],
        }))
    write_layer(INPUTS / "populated_places.geojson", places)


def historical_layer(cache):
    source = download(cache, "cliopatria.zip", HISTORY, HISTORY_HASH)
    items = []
    with ZipFile(source) as archive, archive.open("cliopatria_polities_only.geojson") as data:
        # This pinned release encodes each feature on one line.
        for line in data:
            if not line.lstrip().startswith(b'{ "type": "Feature",'):
                continue
            item = json.loads(line.rstrip().rstrip(b","))
            fields = item["properties"]
            if fields["FromYear"] <= 1440 <= fields["ToYear"]:
                for geometry in clipped(item, ("Polygon", "MultiPolygon")):
                    items.append((geometry, {
                        "name": fields["Name"],
                        "from_year": fields["FromYear"],
                        "to_year": fields["ToYear"],
                    }))
    # Prefer the more local boundary when upstream polities overlap.
    items.sort(key=lambda item: (-item[0].area, item[1]["name"]))
    write_layer(INPUTS / "historical_1440.geojson", items)


def relief_layer(cache):
    source = download(cache, "etopo1.nc", RELIEF)
    with netcdf_file(source, mmap=False) as dataset:
        lat = dataset.variables["latitude"].data.astype("<f8")
        lon = dataset.variables["longitude"].data.astype("<f8")
        altitude = dataset.variables["altitude"].data.astype("<i2")
    actual = sha256(lat.tobytes() + lon.tobytes() + altitude.tobytes()).hexdigest()
    if actual != RELIEF_DATA_HASH:
        raise ValueError(f"{source}: coordinate/elevation checksum mismatch: {actual}")
    np.savez_compressed(INPUTS / "elevation.npz", latitude=lat, longitude=lon, altitude=altitude)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--cache", type=Path, default=Path(tempfile.gettempdir()) / "strategy-map-m2-inputs")
    args = parser.parse_args()
    args.cache.mkdir(parents=True, exist_ok=True)
    INPUTS.mkdir(parents=True, exist_ok=True)
    physical_layers(args.cache)
    populated_places_layer(args.cache)
    historical_layer(args.cache)
    relief_layer(args.cache)
    print(f"Regenerated source subsets in {INPUTS}")


if __name__ == "__main__":
    main()
