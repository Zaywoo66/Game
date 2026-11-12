"""Build a single Europe raster while preserving every Western Europe pixel and ID."""

import argparse
from collections import Counter
from hashlib import sha256
from pathlib import Path
import re

import numpy as np
from PIL import Image
from rasterio.features import rasterize
from rasterio.transform import from_bounds
from scipy import ndimage

from map_common import EDGE_ORDER, ROOT, json_bytes, map_hash, read_json, read_yaml, yaml_bytes
from topology import raster_contacts, regular_edge
from polity_evidence import COARSE_GROUPS

BOUNDS = (-13.0, 36.0, 60.0, 72.0)
WIDTH, HEIGHT = 2336, 1152
WEST_Y = 408
WEST_WIDTH, WEST_HEIGHT = 800, 744
SHAPE = (HEIGHT, WIDTH)
INPUTS = Path(__file__).resolve().parent / "inputs" / "europe"
WEST = ROOT / "data" / "map" / "western_europe"
OUTPUT = ROOT / "data" / "map" / "europe"
PROJECTION = {"pixels_per_degree": 32, "origin": [5344, 576]}
SOURCE_URL = "https://github.com/Seshat-Global-History-Databank/cliopatria/releases/tag/v0.1.3"


def layer(name):
    return read_json(INPUTS / f"{name}.geojson")["features"]


def decode_ids(path):
    rgb = np.asarray(Image.open(path), dtype=np.uint32)
    return (rgb[..., 0] << 16) | (rgb[..., 1] << 8) | rgb[..., 2]


def point_pixel(point):
    lon, lat = point
    return int((72 - lat) * 32), int((lon + 13) * 32)


def western_rect():
    result = np.zeros(SHAPE, dtype=bool)
    result[WEST_Y:, :WEST_WIDTH] = True
    return result


def build():
    west = read_json(WEST / "provinces.json")
    west_records = west["provinces"]
    old_ids = decode_ids(WEST / "provinces.png")
    assert old_ids.shape == (WEST_HEIGHT, WEST_WIDTH)
    transform = from_bounds(*BOUNDS, WIDTH, HEIGHT)
    land = rasterize(((f["geometry"], 1) for f in layer("land")), out_shape=SHAPE,
                     transform=transform, dtype=np.uint8).astype(bool)
    lakes = rasterize(((f["geometry"], 1) for f in layer("lakes")), out_shape=SHAPE,
                      transform=transform, dtype=np.uint8).astype(bool) & land
    fixed = western_rect()
    west_terrain = np.asarray(Image.open(WEST / "terrain.png"))
    land[WEST_Y:, :WEST_WIDTH] = np.isin(west_terrain, (1, 2, 3))
    lakes[WEST_Y:, :WEST_WIDTH] = west_terrain == 4
    sea = ~land & ~lakes
    rivers = rasterize(((f["geometry"], 1) for f in layer("rivers")), out_shape=SHAPE,
                       transform=transform, all_touched=True, dtype=np.uint8).astype(bool) & land
    rivers[WEST_Y:, :WEST_WIDTH] = np.asarray(Image.open(WEST / "rivers.png")) != 0
    kinds = {record["id"]: record["kind"] for record in west_records}
    used_keys = {record["key"] for record in west_records}
    names = read_json(WEST / "names.en.json")
    records = {record["id"]: {**record, "centroid": [record["centroid"][0],
                                                     record["centroid"][1] + WEST_Y]}
               for record in west_records}
    ids = np.zeros(SHAPE, dtype=np.uint32)
    ids[WEST_Y:, :WEST_WIDTH] = old_ids
    next_id = max(records) + 1
    seed_points = {}

    def add_seed(key, name, kind, y, x):
        nonlocal next_id
        if key in used_keys or not (0 <= y < HEIGHT and 0 <= x < WIDTH) or fixed[y, x]:
            return False
        mask = {"land": land, "sea": sea, "lake": lakes}[kind] & ~fixed
        top, bottom = max(0, y - 12), min(HEIGHT, y + 13)
        left, right = max(0, x - 12), min(WIDTH, x + 13)
        candidates = np.argwhere(mask[top:bottom, left:right])
        if not len(candidates):
            return False
        candidates += (top, left)
        y, x = map(int, min(candidates, key=lambda p: ((p[0]-y)**2 + (p[1]-x)**2, p[0], p[1])))
        if any((y-a)**2 + (x-b)**2 < (28 if kind == "land" else 64)**2
               for _, (a, b, other) in seed_points.items() if other == kind):
            return False
        while key in used_keys:
            key += "_2"
        used_keys.add(key)
        kinds[next_id] = kind
        seed_points[next_id] = (y, x, kind)
        names[f"province_{key}"] = name
        records[next_id] = {
            "id": next_id, "key": key, "kind": kind,
            "terrain": "plains" if kind == "land" else kind,
            "centroid": [x, y], "neighbors": [], "ports": [], "development": 0,
            "name_key": f"province_{key}",
            "area": "europe_extension_water" if kind != "land" else "europe_extension_unclassified",
        }
        next_id += 1
        return True

    places = sorted(layer("populated_places"), key=lambda f: (
        f["properties"]["rank"], -f["properties"]["population"], f["properties"]["name"]
    ))
    for place in places:
        name = place["properties"]["name"]
        key = re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")
        if not key or key[0].isdigit():
            key = "city_" + key
        if key in used_keys:
            continue
        add_seed(key, name, "land", *point_pixel(place["geometry"]["coordinates"]))

    # Sparse fallback cells cover the northern and eastern gaps in the place layer.
    for y in range(20, HEIGHT, 48):
        for x in range(20, WIDTH, 48):
            lon, lat = -13 + (x + .5) / 32, 72 - (y + .5) / 32
            add_seed(f"eu_{int((lon+13)*10):03d}_{int((72-lat)*10):03d}",
                     f"{lat:.1f}°N, {lon:.1f}°E", "land", y, x)
    for y in range(32, HEIGHT, 112):
        for x in range(32, WIDTH, 112):
            add_seed(f"sea_{y:04d}_{x:04d}", f"Sea zone {y}, {x}", "sea", y, x)
    components, count = ndimage.label(lakes & ~fixed)
    for component in range(1, count + 1):
        pixels = np.argwhere(components == component)
        if len(pixels) >= 3:
            y, x = map(int, pixels[len(pixels)//2])
            add_seed(f"lake_{component:03d}", f"Lake {component}", "lake", y, x)
    if next_id > 2000:
        raise ValueError(f"Europe ID reservation exceeded: {next_id - 1}")

    for kind, mask in (("land", land), ("sea", sea), ("lake", lakes)):
        seed_mask = np.ones(SHAPE, dtype=bool)
        lookup = np.zeros(SHAPE, dtype=np.uint32)
        for province, (y, x, seed_kind) in seed_points.items():
            if seed_kind == kind:
                seed_mask[y, x] = False
                lookup[y, x] = province
        if np.all(seed_mask):
            raise ValueError(f"no {kind} seeds")
        nearest = ndimage.distance_transform_edt(seed_mask, return_distances=False, return_indices=True)
        target = mask & ~fixed
        ids[target] = lookup[nearest[0][target], nearest[1][target]]
    if np.any(ids == 0):
        raise ValueError("unassigned Europe pixels")

    terrain = np.where(sea, 0, np.where(lakes, 4, 1)).astype(np.uint8)
    terrain[WEST_Y:, :WEST_WIDTH] = west_terrain
    with np.load(INPUTS / "elevation.npz") as relief:
        lat, lon = relief["latitude"], relief["longitude"]
        sample_y = np.interp(72 - (np.arange(HEIGHT) + .5) / 32, lat, np.arange(len(lat)))
        sample_x = np.interp(-13 + (np.arange(WIDTH) + .5) / 32, lon, np.arange(len(lon)))
        height_m = ndimage.map_coordinates(
            relief["altitude"].astype(np.float64),
            np.broadcast_arrays(sample_y[:, None], sample_x[None, :]), order=1, mode="nearest",
        )
    elevation = (np.rint(height_m).astype(np.int32) + 32768).astype(np.uint16)
    terrain[land & (height_m >= 350)] = 2
    terrain[land & (height_m >= 1400)] = 3
    elevation[WEST_Y:, :WEST_WIDTH] = np.asarray(Image.open(WEST / "heightmap.png"))
    coordinates = ndimage.center_of_mass(np.ones(SHAPE), ids, list(seed_points))
    for province, center in zip(seed_points, coordinates, strict=True):
        cy, cx = map(lambda value: int(round(value)), center)
        if 0 <= cy < HEIGHT and 0 <= cx < WIDTH and ids[cy, cx] == province:
            records[province]["centroid"] = [cx, cy]
        else:
            y, x, _ = seed_points[province]
            records[province]["centroid"] = [x, y]
        py, px = np.where(ids == province)
        if not len(py):
            raise ValueError(f"seed province {province} disappeared")
        majority = np.bincount(terrain[py, px]).argmax()
        records[province]["terrain"] = ("sea", "plains", "hills", "mountains", "lake")[majority]

    contacts, river_contacts = raster_contacts(ids, rivers)
    edges = {province: set() for province in records}
    for pair in contacts:
        edge = regular_edge(pair, contacts, river_contacts, kinds)
        a, b = pair
        edges[a].add((b, edge))
        edges[b].add((a, edge))
    for record in west_records:
        for sea_id in record["ports"]:
            edges[record["id"]].add((sea_id, "port"))
            edges[sea_id].add((record["id"], "port"))
    costs = read_yaml(Path(__file__).resolve().parent / "inputs" / "western_europe" / "overrides.yaml")["movement_costs"]
    for province, record in records.items():
        record["neighbors"] = [{"id": other, "edge": edge, "cost": costs[edge]}
                               for other, edge in sorted(edges[province],
                                                         key=lambda e: (e[0], EDGE_ORDER.index(e[1])))]
    provinces = [records[province] for province in sorted(records)]
    metadata = {"schema_version": 3, "map_version": 4, "map_hash": map_hash(provinces, projection=PROJECTION),
                "projection": PROJECTION, "provinces": provinces}
    history_features = layer("historical_1440")
    history = rasterize(((f["geometry"], i) for i, f in enumerate(history_features, 1)),
                        out_shape=SHAPE, transform=transform, dtype=np.uint16)
    assignments = []
    for record in provinces:
        if record["kind"] != "land":
            continue
        values = history[ids == record["id"]]
        counts = Counter()
        for feature_id, amount in enumerate(np.bincount(values)):
            if feature_id and amount:
                counts[history_features[feature_id - 1]["properties"]["name"]] += int(amount)
        name = min(counts, key=lambda value: (-counts[value], value)) if counts else None
        assignments.append({"province_id": record["id"], "province_key": record["key"],
                            "polity_name": name,
                            "resolution": "unmatched" if name is None else "coarse_group" if name in COARSE_GROUPS else "polygon",
                            "overlap_pixels": counts[name] if name else 0, "province_pixels": len(values)})
    evidence = {"schema_version": 1, "map_version": 4, "map_hash": metadata["map_hash"],
                "reference_year": 1440,
                "source": {"name": "Cliopatria", "release": "v0.1.3", "url": SOURCE_URL,
                           "license": "CC BY 4.0", "file": "tools/map/inputs/europe/historical_1440.geojson",
                           "sha256": sha256((INPUTS / "historical_1440.geojson").read_bytes().replace(b"\r\n", b"\n")).hexdigest()},
                "method": "largest rasterized source-polygon pixel overlap; lexical tie break",
                "assignments": assignments}
    areas = read_yaml(WEST / "areas.yaml")
    areas["areas"].extend([
        {"key": "europe_extension_unclassified", "name_key": "area_europe_extension_unclassified",
         "name": "Europe extension (unclassified)", "region": "europe_extension", "kind": "unclassified", "source": None},
        {"key": "europe_extension_water", "name_key": "area_europe_extension_water",
         "name": "Europe extension waters", "region": "europe_extension", "kind": "water", "source": None},
    ])
    regions = read_yaml(WEST / "regions.yaml")
    regions["regions"].append({"key": "europe_extension", "name_key": "region_europe_extension", "name": "Europe extension"})
    names.update({"area_europe_extension_unclassified": "Europe extension (unclassified)",
                  "area_europe_extension_water": "Europe extension waters", "region_europe_extension": "Europe extension"})
    OUTPUT.mkdir(parents=True, exist_ok=True)
    rgb = np.stack((ids >> 16, ids >> 8, ids), axis=-1).astype(np.uint8)
    Image.fromarray(rgb).save(OUTPUT / "provinces.png", compress_level=9)
    Image.fromarray(elevation).save(OUTPUT / "heightmap.png", compress_level=9)
    Image.fromarray(terrain).save(OUTPUT / "terrain.png", compress_level=9)
    Image.fromarray(rivers.astype(np.uint8) * 255).save(OUTPUT / "rivers.png", compress_level=9)
    (OUTPUT / "provinces.json").write_bytes(json_bytes(metadata, pretty=True))
    (OUTPUT / "polities_1440.json").write_bytes(json_bytes(evidence, pretty=True))
    (OUTPUT / "areas.yaml").write_bytes(yaml_bytes(areas))
    (OUTPUT / "regions.yaml").write_bytes(yaml_bytes(regions))
    (OUTPUT / "names.en.json").write_bytes(json_bytes(names, pretty=True))
    scenario_path = ROOT / "data" / "scenarios" / "1440.json"
    scenario = read_json(scenario_path)
    owned = set()
    for tag in scenario["countries"]:
        country = read_json(ROOT / "data" / "countries" / f"{tag}.json")
        owned.update(country["start"]["provinces"])
    neutrals = set(scenario["unowned_provinces"]) - owned
    western_province_ids = {p["id"] for p in west_records}
    neutrals.update(record["key"] for record in provinces
                    if record["kind"] == "land" and record["id"] not in western_province_ids
                    and record["key"] not in owned)
    scenario["unowned_provinces"] = sorted(neutrals)
    scenario_path.write_bytes(json_bytes(scenario, pretty=True))
    print(f"{len(provinces)} provinces: {dict(Counter(p['kind'] for p in provinces))}; hash {metadata['map_hash']}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--region", choices=("europe",), required=True)
    parser.parse_args()
    build()
