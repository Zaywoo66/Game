"""Append a North Africa / Middle East raster extension without renumbering Europe."""

from __future__ import annotations

from collections import Counter
import json
import re

import numpy as np
from PIL import Image
from rasterio.features import rasterize
from rasterio.transform import from_bounds
from scipy import ndimage

import build_modern_scenario as modern
from map_common import ROOT, json_bytes, map_hash, read_json, read_yaml, yaml_bytes
from topology import raster_contacts, regular_edge


OUT = ROOT / "data/map/europe"
OLD_BOUNDS = (-13.0, 36.0, 68.0, 72.0)
BOUNDS = (-18.0, 10.0, 68.0, 72.0)
PPD = 32
WIDTH, HEIGHT = int((BOUNDS[2] - BOUNDS[0]) * PPD), int((BOUNDS[3] - BOUNDS[1]) * PPD)
X_SHIFT = int((OLD_BOUNDS[0] - BOUNDS[0]) * PPD)
OLD_HEIGHT, OLD_WIDTH = 1152, 2592
NEW_ID_START = 4000


def pix(lon: float, lat: float) -> tuple[int, int]:
    return int((BOUNDS[3] - lat) * PPD), int((lon - BOUNDS[0]) * PPD)


def key_for(name: str, used: set[str]) -> str:
    key = re.sub(r"[^a-z0-9]+", "_", name.lower()).strip("_")
    if not key or key[0].isdigit():
        key = "place_" + key
    while key in used:
        key += "_place"
    used.add(key)
    return key


def main() -> None:
    old = read_json(modern.MAP_JSON)
    old_pixels = np.asarray(Image.open(modern.MAP_IMAGE).convert("RGB"), dtype=np.uint32)
    old_ids = (old_pixels[..., 0] << 16) | (old_pixels[..., 1] << 8) | old_pixels[..., 2]
    if old_ids.shape != (OLD_HEIGHT, OLD_WIDTH) or old["map_version"] != 7:
        raise ValueError("expected the checked-in v7 full-Europe map")

    modern.BOUNDS = BOUNDS
    modern.WIDTH, modern.HEIGHT = WIDTH, HEIGHT
    modern.SHAPE = (HEIGHT, WIDTH)
    groups, _, places = modern.read_groups()
    transform = from_bounds(*BOUNDS, WIDTH, HEIGHT)
    land_source = rasterize(((g["geometry"], 1) for g in groups), out_shape=(HEIGHT, WIDTH),
                            transform=transform, dtype=np.uint8).astype(bool)
    ids = np.zeros((HEIGHT, WIDTH), dtype=np.uint32)
    terrain = np.zeros((HEIGHT, WIDTH), dtype=np.uint8)
    rivers = np.zeros((HEIGHT, WIDTH), dtype=np.uint8)
    height = np.full((HEIGHT, WIDTH), 32768, dtype=np.uint16)
    old_terrain = np.asarray(Image.open(OUT / "terrain.png"))
    old_height = np.asarray(Image.open(OUT / "heightmap.png"))
    old_rivers = np.asarray(Image.open(OUT / "rivers.png"))
    old_ids_shifted = np.zeros_like(ids)
    old_ids_shifted[:OLD_HEIGHT, X_SHIFT:X_SHIFT + OLD_WIDTH] = old_ids
    ids[:OLD_HEIGHT, X_SHIFT:X_SHIFT + OLD_WIDTH] = old_ids
    terrain[:OLD_HEIGHT, X_SHIFT:X_SHIFT + OLD_WIDTH] = old_terrain
    height[:OLD_HEIGHT, X_SHIFT:X_SHIFT + OLD_WIDTH] = old_height
    rivers[:OLD_HEIGHT, X_SHIFT:X_SHIFT + OLD_WIDTH] = old_rivers
    land_source[:OLD_HEIGHT, X_SHIFT:X_SHIFT + OLD_WIDTH] = np.isin(old_terrain, (1, 2, 3))
    sea = ~land_source
    extension = np.ones((HEIGHT, WIDTH), dtype=bool)
    extension[:OLD_HEIGHT, X_SHIFT:X_SHIFT + OLD_WIDTH] = False

    rows = {int(row["id"]): dict(row) for row in old["provinces"]}
    keys = {row["key"] for row in rows.values()}
    for row in rows.values():
        row["centroid"] = [row["centroid"][0] + X_SHIFT, row["centroid"][1]]

    points: dict[int, tuple[int, int, str]] = {}
    names = read_json(OUT / "names.en.json")
    next_id = NEW_ID_START

    def seed(name: str, kind: str, y: int, x: int, *, spacing: int) -> bool:
        nonlocal next_id
        mask = (land_source if kind == "land" else sea) & extension
        if not (0 <= y < HEIGHT and 0 <= x < WIDTH) or not mask[y, x]:
            top, bottom = max(0, y - 20), min(HEIGHT, y + 21)
            left, right = max(0, x - 20), min(WIDTH, x + 21)
            options = np.argwhere(mask[top:bottom, left:right])
            if not len(options):
                return False
            options += (top, left)
            y, x = map(int, min(options, key=lambda p: ((p[0] - y) ** 2 + (p[1] - x) ** 2, p[0], p[1])))
        if any(k == kind and (y - sy) ** 2 + (x - sx) ** 2 < spacing**2
               for sy, sx, k in points.values()):
            return False
        if next_id >= 6000:
            raise ValueError("North Africa ID range 4000-5999 exceeded")
        key = key_for(name, keys)
        pid = next_id
        next_id += 1
        points[pid] = (y, x, kind)
        rows[pid] = {
            "id": pid, "key": key, "kind": kind,
            "terrain": "plains" if kind == "land" else "sea",
            "centroid": [x, y], "neighbors": [], "ports": [], "development": 0,
            "name_key": f"province_{key}",
            "area": "africa_middle_east_unclassified" if kind == "land" else "africa_middle_east_water",
        }
        names[f"province_{key}"] = name
        return True

    # Place seeds retain recognizable city names; synthetic points only fill deserts and open water.
    for place in sorted(places, key=lambda p: (not p["is_capital"], -p["population"], p["name"])):
        y, x = pix(place["x"], place["y"])
        if extension[max(0, min(HEIGHT - 1, y)), max(0, min(WIDTH - 1, x))]:
            seed(place["name"], "land", y, x, spacing=40)
    for y in range(24, HEIGHT, 72):
        for x in range(24, WIDTH, 72):
            if extension[y, x] and land_source[y, x]:
                lon, lat = BOUNDS[0] + x / PPD, BOUNDS[3] - y / PPD
                seed(f"atlas_{int(lon * 10):04d}_{int(lat * 10):04d}", "land", y, x, spacing=46)
    for y in range(48, HEIGHT, 128):
        for x in range(48, WIDTH, 128):
            if extension[y, x] and sea[y, x]:
                seed(f"sea_africa_{y:04d}_{x:04d}", "sea", y, x, spacing=80)

    for kind, mask in (("land", land_source), ("sea", sea)):
        target = mask & extension
        seed_mask = np.ones((HEIGHT, WIDTH), dtype=bool)
        seed_ids = np.zeros((HEIGHT, WIDTH), dtype=np.uint32)
        for pid, (y, x, seed_kind) in points.items():
            if seed_kind == kind:
                seed_mask[y, x] = False
                seed_ids[y, x] = pid
        nearest = ndimage.distance_transform_edt(seed_mask, return_distances=False, return_indices=True)
        ids[target] = seed_ids[nearest[0][target], nearest[1][target]]
        if np.any(target & (ids == 0)):
            raise ValueError(f"unassigned {kind} pixels in extension")
        terrain[target] = 1 if kind == "land" else 0
    if np.any(ids == 0):
        raise ValueError("map raster contains reserved province ID zero")

    for pid, (y, x, _) in points.items():
        py, px = np.where(ids == pid)
        if len(py) < 2:
            raise ValueError(f"generated province {rows[pid]['key']} is too small")
        rows[pid]["centroid"] = [int(np.mean(px)), int(np.mean(py))]
        rows[pid]["terrain"] = "plains" if rows[pid]["kind"] == "land" else "sea"

    costs = read_yaml(ROOT / "tools/map/inputs/western_europe/overrides.yaml")["movement_costs"]
    kinds = {pid: row["kind"] for pid, row in rows.items()}
    contacts, river_contacts = raster_contacts(ids, rivers != 0)
    edge_sets = {pid: {(edge["id"], edge["edge"]): edge["cost"] for edge in row["neighbors"]}
                 for pid, row in rows.items()}
    for pair in contacts:
        if pair[0] < NEW_ID_START and pair[1] < NEW_ID_START:
            continue
        edge = regular_edge(pair, contacts, river_contacts, kinds)
        for src, dst in ((pair[0], pair[1]), (pair[1], pair[0])):
            edge_sets[src][(dst, edge)] = costs[edge]
    for pid, row in rows.items():
        row["neighbors"] = [
            {"id": other, "edge": edge, "cost": cost}
            for (other, edge), cost in sorted(edge_sets[pid].items(), key=lambda x: (x[0][0], x[0][1]))
        ]

    for pid, row in rows.items():
        if pid >= NEW_ID_START:
            continue
        row["ports"] = [p for p in row["ports"]]
    provinces = [rows[pid] for pid in sorted(rows)]
    projection = {"pixels_per_degree": PPD, "origin": [5184, 576]}
    metadata = {"schema_version": 3, "map_version": 8,
                "map_hash": map_hash(provinces, projection=projection),
                "projection": projection, "provinces": provinces}

    areas = read_yaml(OUT / "areas.yaml")
    regions = read_yaml(OUT / "regions.yaml")
    areas["areas"].append({"key": "africa_middle_east_unclassified", "name_key": "area_africa_middle_east_unclassified",
                           "name": "Africa and Middle East (unclassified)", "region": "africa_middle_east", "kind": "unclassified", "source": None})
    areas["areas"].append({"key": "africa_middle_east_water", "name_key": "area_africa_middle_east_water",
                           "name": "Africa and Middle East waters", "region": "africa_middle_east", "kind": "water", "source": None})
    regions["regions"].append({"key": "africa_middle_east", "name_key": "region_africa_middle_east", "name": "Africa and Middle East"})
    names.update({"area_africa_middle_east_unclassified": "Africa and Middle East (unclassified)",
                  "area_africa_middle_east_water": "Africa and Middle East waters",
                  "region_africa_middle_east": "Africa and Middle East"})

    Image.fromarray(np.stack((ids >> 16, ids >> 8, ids), axis=-1).astype(np.uint8)).save(OUT / "provinces.png", compress_level=9)
    Image.fromarray(terrain).save(OUT / "terrain.png", compress_level=9)
    Image.fromarray(height).save(OUT / "heightmap.png", compress_level=9)
    Image.fromarray(rivers).save(OUT / "rivers.png", compress_level=9)
    (OUT / "provinces.json").write_bytes(json_bytes(metadata, pretty=True))
    (OUT / "areas.yaml").write_bytes(yaml_bytes(areas))
    (OUT / "regions.yaml").write_bytes(yaml_bytes(regions))
    (OUT / "names.en.json").write_bytes(json_bytes(names, pretty=True))

    evidence = read_json(OUT / "polities_1440.json")
    pixels_by_id = dict(zip(*np.unique(ids, return_counts=True)))
    for pid, row in rows.items():
        if pid >= NEW_ID_START and row["kind"] == "land":
            evidence["assignments"].append({"province_id": pid, "province_key": row["key"], "polity_name": None,
                                            "resolution": "unmatched", "overlap_pixels": 0,
                                            "province_pixels": int(pixels_by_id[pid])})
    evidence["map_version"], evidence["map_hash"] = 8, metadata["map_hash"]
    (OUT / "polities_1440.json").write_bytes(json_bytes(evidence, pretty=True))
    ownership = read_json(OUT / "ownership_1440.json")
    ownership["map_version"], ownership["map_hash"] = 8, metadata["map_hash"]
    (OUT / "ownership_1440.json").write_bytes(json_bytes(ownership, pretty=True))
    scenario_path = ROOT / "data/scenarios/1440.json"
    scenario = read_json(scenario_path)
    already = set(scenario["unowned_provinces"])
    scenario["unowned_provinces"] = sorted(already | {r["key"] for r in rows.values()
                                                        if r["id"] >= NEW_ID_START and r["kind"] == "land"})
    scenario_path.write_bytes(json_bytes(scenario, pretty=True))
    print(f"appended {sum(r['id'] >= NEW_ID_START for r in provinces)} provinces; land extension unowned in legacy 1440")
    print(f"map v8 {WIDTH}x{HEIGHT}; {len(provinces)} provinces; hash {metadata['map_hash']}")


if __name__ == "__main__":
    main()
