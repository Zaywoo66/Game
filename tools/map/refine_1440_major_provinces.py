"""Refine the 1440 Europe raster inside owner-approved major-country cells."""

from __future__ import annotations

import json
import math
from collections import Counter
from hashlib import sha256
from pathlib import Path

import numpy as np
from PIL import Image
from rasterio.features import rasterize
from rasterio.transform import from_bounds
from scipy import ndimage

from build_europe import BOUNDS, INPUTS, OUTPUT, PROJECTION, ROOT, decode_ids, layer
from map_common import EDGE_ORDER, json_bytes, map_hash, read_json
from topology import raster_contacts, regular_edge

CONFIG = INPUTS / "major_density_1440.json"
SCENARIO_PATH = ROOT / "data/scenarios/1440.json"
COUNTRY_DIR = ROOT / "data/countries"
OWNERSHIP_POLICY_PATH = ROOT / "data/map/europe/ownership_1440.json"
OUTPUT_MAP_VERSION = 7
EUROPE_ID_LIMIT = 4000
TERRAIN_NAMES = ("sea", "plains", "hills", "mountains", "lake")
MOVEMENT_COSTS = {"normal": 3, "river": 5, "strait": 5, "port": 2}


def read_config(path: Path = CONFIG) -> dict:
    return json.loads(path.read_text(encoding="utf-8"))


def physical_weights(height: int) -> np.ndarray:
    """Approximate pixel ground area by cos(latitude) for the equirectangular map."""
    latitudes = 72.0 - (np.arange(height, dtype=np.float64) + 0.5) / 32.0
    return np.cos(np.deg2rad(latitudes))


def farthest_seeds(ys: np.ndarray, xs: np.ndarray, count: int, first: tuple[int, int]) -> list[tuple[int, int]]:
    """Deterministic farthest-point sampling; ties use raster y then x order."""
    if count < 1 or count > len(ys):
        raise ValueError("invalid child count for province raster")
    seed_y, seed_x = first
    if not np.any((ys == seed_y) & (xs == seed_x)):
        raise ValueError("parent centroid is outside its raster mask")
    seeds = [(seed_y, seed_x)]
    min_distance = np.full(len(ys), np.inf, dtype=np.float64)
    reference_latitude = 72.0 - (float(np.mean(ys)) + 0.5) / 32.0
    east_scale = math.cos(math.radians(reference_latitude))
    while len(seeds) < count:
        y, x = seeds[-1]
        dy = ys.astype(np.float64) - y
        dx = (xs.astype(np.float64) - x) * east_scale
        np.minimum(min_distance, dy * dy + dx * dx, out=min_distance)
        best = int(np.argmax(min_distance))
        if not np.isfinite(min_distance[best]) or min_distance[best] == 0:
            raise ValueError("could not place distinct child seeds")
        seeds.append((int(ys[best]), int(xs[best])))
    return seeds


def split_mask(
    parent_id: int,
    parent: dict,
    ys: np.ndarray,
    xs: np.ndarray,
    target: int,
    row_weights: np.ndarray,
) -> tuple[list[tuple[int, int]], np.ndarray]:
    """Split one parent mask into bounded Euclidean Voronoi children."""
    weights = row_weights[ys]
    physical_area = float(weights.sum())
    count = max(1, math.ceil(physical_area / target))
    if count == 1:
        return [(int(parent["centroid"][1]), int(parent["centroid"][0]))], np.zeros(len(ys), dtype=np.uint32)
    first = (int(parent["centroid"][1]), int(parent["centroid"][0]))
    seeds = farthest_seeds(ys, xs, count, first)
    center_latitude = 72.0 - (float(np.mean(ys)) + 0.5) / 32.0
    east_scale = math.cos(math.radians(center_latitude))
    best_distance = np.full(len(ys), np.inf, dtype=np.float64)
    labels = np.zeros(len(ys), dtype=np.uint32)
    for index, (seed_y, seed_x) in enumerate(seeds):
        dy = ys.astype(np.float64) - seed_y
        dx = (xs.astype(np.float64) - seed_x) * east_scale
        distance = dy * dy + dx * dx
        better = distance < best_distance
        labels[better] = index
        best_distance[better] = distance[better]
    if len(np.unique(labels)) != count:
        raise ValueError(f"province {parent_id}: a child received no raster pixels")
    return seeds, labels


def _pixel_groups(ids: np.ndarray) -> dict[int, np.ndarray]:
    """Build sorted raster positions per province without rescanning the whole image."""
    flat = ids.ravel()
    positions = np.flatnonzero(flat)
    order = np.argsort(flat[positions], kind="stable")
    sorted_positions = positions[order]
    sorted_ids = flat[sorted_positions]
    values, starts = np.unique(sorted_ids, return_index=True)
    ends = np.append(starts[1:], len(sorted_positions))
    return {int(value): sorted_positions[start:end] for value, start, end in zip(values, starts, ends, strict=True)}


def refine(
    map_data: dict,
    ids: np.ndarray,
    terrain: np.ndarray,
    rivers: np.ndarray,
    evidence: dict,
    owners: dict[str, str],
    names: dict[str, str],
    config: dict,
    policy: dict | None = None,
) -> tuple[dict, np.ndarray, dict, dict, dict[str, str], dict]:
    """Return a refined map and updated evidence/ownership without writing files."""
    expected = (config["input_map_version"], config["input_map_hash"])
    actual = (map_data["map_version"], map_data["map_hash"])
    if actual != expected:
        raise ValueError(f"density input map identity mismatch: expected {expected}, found {actual}")
    if ids.shape != terrain.shape or ids.shape != rivers.shape:
        raise ValueError("map raster layers must have identical dimensions")
    tags = set(config["refined_country_tags"])
    missing = sorted(tags - set(owners.values()))
    if missing:
        raise ValueError(f"major-country density policy references countries without provinces: {missing}")
    rows = {record["id"]: dict(record) for record in map_data["provinces"]}
    names = dict(names)
    groups = _pixel_groups(ids)
    row_weights = physical_weights(ids.shape[0])
    for province_id, row in rows.items():
        x, y = row["centroid"]
        if not (0 <= y < ids.shape[0] and 0 <= x < ids.shape[1]) or ids[y, x] != province_id:
            positions = groups[province_id]
            ys, xs = np.unravel_index(positions, ids.shape)
            nearest = int(np.argmin((ys - y) ** 2 + (xs - x) ** 2))
            row["centroid"] = [int(xs[nearest]), int(ys[nearest])]
    max_id = max(rows)
    child_groups: dict[int, list[int]] = {}
    allocation_id = max_id + 1

    for parent_id in sorted(rows):
        parent = rows[parent_id]
        tag = owners.get(parent["key"])
        if parent["kind"] != "land" or tag not in tags:
            continue
        positions = groups[parent_id]
        ys, xs = np.unravel_index(positions, ids.shape)
        seeds, labels = split_mask(
            parent_id, parent, ys, xs, config["target_physical_pixel_area"], row_weights
        )
        if len(seeds) == 1:
            child_groups[parent_id] = [parent_id]
            continue
        child_ids = [parent_id]
        for _ in seeds[1:]:
            child_ids.append(allocation_id)
            allocation_id += 1
        child_groups[parent_id] = child_ids
        ids[ys, xs] = np.asarray(child_ids, dtype=np.uint32)[labels]

        parent_name = names[parent["name_key"]]
        development, remainder = divmod(parent["development"], len(child_ids))
        for index, ((seed_y, seed_x), child_id) in enumerate(zip(seeds[1:], child_ids[1:], strict=True), start=2):
            key = f"{parent['key']}_part_{index:02}"
            if key in names or any(row["key"] == key for row in rows.values()):
                raise ValueError(f"generated province key already exists: {key}")
            child_mask = labels == index - 1
            child_ys, child_xs = ys[child_mask], xs[child_mask]
            center_y = float(child_ys.mean())
            center_x = float(child_xs.mean())
            nearest = int(np.argmin((child_ys - center_y) ** 2 + (child_xs - center_x) ** 2))
            terrain_index = int(np.bincount(terrain[child_ys, child_xs]).argmax())
            name_key = f"province_{key}"
            display_name = f"{parent_name} {index}"
            names[name_key] = display_name
            rows[child_id] = {
                "id": child_id,
                "key": key,
                "kind": "land",
                "terrain": TERRAIN_NAMES[terrain_index],
                "centroid": [int(child_xs[nearest]), int(child_ys[nearest])],
                "neighbors": [],
                "ports": [],
                "development": development + int(index - 2 < remainder),
                "name_key": name_key,
                "area": parent["area"],
            }
        # Keep the original settlement/key/ID on its centroid child.
        first_mask = labels == 0
        first_ys, first_xs = ys[first_mask], xs[first_mask]
        center_y = float(first_ys.mean())
        center_x = float(first_xs.mean())
        nearest = int(np.argmin((first_ys - center_y) ** 2 + (first_xs - center_x) ** 2))
        parent["centroid"] = [int(first_xs[nearest]), int(first_ys[nearest])]
        parent["development"] = development
        terrain_index = int(np.bincount(terrain[first_ys, first_xs]).argmax())
        parent["terrain"] = TERRAIN_NAMES[terrain_index]

    kinds = {province_id: record["kind"] for province_id, record in rows.items()}
    contacts, river_contacts = raster_contacts(ids, rivers != 0)
    edges: dict[int, set[tuple[int, str]]] = {province_id: set() for province_id in rows}
    for pair in contacts:
        edge = regular_edge(pair, contacts, river_contacts, kinds)
        first, second = pair
        edges[first].add((second, edge))
        edges[second].add((first, edge))

    # Preserve explicit sea crossings; original IDs remain on their centroid children.
    for old in map_data["provinces"]:
        for edge in old["neighbors"]:
            if edge["edge"] == "strait":
                edges[old["id"]].add((edge["id"], "strait"))

    port_sets: dict[int, set[int]] = {province_id: set() for province_id in rows}
    for old in map_data["provinces"]:
        if not old["ports"]:
            continue
        candidates = child_groups.get(old["id"], [old["id"]])
        for sea_id in old["ports"]:
            with_contact = [
                (contacts.get(tuple(sorted((candidate, sea_id))), 0), candidate)
                for candidate in candidates
            ]
            chosen = min(with_contact, key=lambda item: (-item[0], item[1]))[1]
            port_sets[chosen].add(sea_id)
            edges[chosen].add((sea_id, "port"))
            edges[sea_id].add((chosen, "port"))

    for province_id, row in rows.items():
        row["ports"] = sorted(port_sets[province_id])
        row["neighbors"] = [
            {"id": other, "edge": edge, "cost": MOVEMENT_COSTS[edge]}
            for other, edge in sorted(edges[province_id], key=lambda item: (item[0], EDGE_ORDER.index(item[1])))
        ]

    provinces = [rows[province_id] for province_id in sorted(rows)]
    digest = map_hash(provinces, projection=PROJECTION)
    refined_map = {
        "schema_version": map_data["schema_version"],
        "map_version": config["output_map_version"],
        "map_hash": digest,
        "projection": map_data["projection"],
        "provinces": provinces,
    }

    history_features = layer("historical_1440")
    history = rasterize(
        ((feature["geometry"], index) for index, feature in enumerate(history_features, 1)),
        out_shape=ids.shape,
        transform=from_bounds(*BOUNDS, ids.shape[1], ids.shape[0]),
        dtype=np.uint16,
    )
    assignments = []
    for province in provinces:
        if province["kind"] != "land":
            continue
        pixels = history[ids == province["id"]]
        counts = Counter()
        for feature_id, amount in enumerate(np.bincount(pixels)):
            if feature_id and amount:
                counts[history_features[feature_id - 1]["properties"]["name"]] += int(amount)
        name = min(counts, key=lambda candidate: (-counts[candidate], candidate)) if counts else None
        assignments.append({
            "province_id": province["id"],
            "province_key": province["key"],
            "polity_name": name,
            "resolution": "unmatched" if name is None else "coarse_group" if name in {
                "Holy Roman Empire Minor States", "Kalmar Union", "(Kalmar Union)"
            } else "polygon",
            "overlap_pixels": counts[name] if name is not None else 0,
            "province_pixels": len(pixels),
        })
    refined_evidence = {
        "schema_version": 1,
        "map_version": refined_map["map_version"],
        "map_hash": digest,
        "reference_year": 1440,
        "source": {
            "name": "Cliopatria",
            "release": "v0.2.1",
            "url": "https://github.com/Seshat-Global-History-Databank/cliopatria/releases/tag/v0.2.1",
            "license": "CC BY 4.0",
            "file": "tools/map/inputs/europe/historical_1440.geojson",
            "sha256": sha256((INPUTS / "historical_1440.geojson").read_bytes().replace(b"\r\n", b"\n")).hexdigest(),
        },
        "method": "largest rasterized source-polygon pixel overlap; documented Bavaria-to-Burgundian-Netherlands correction; lexical tie break",
        "assignments": assignments,
    }

    # The policy pins its final map while retaining the base identity used before refinement.
    policy = dict(policy) if policy is not None else read_json(OWNERSHIP_POLICY_PATH)
    policy_identity = (policy.get("map_version"), policy.get("map_hash"))
    policy_base_identity = (
        policy.get("base_map_version", policy.get("map_version")),
        policy.get("base_map_hash", policy.get("map_hash")),
    )
    if policy_identity not in {
        expected,
        (config["output_map_version"], digest),
    } and policy_base_identity != expected:
        raise ValueError("ownership policy map identity does not match base or refined map")
    policy["base_map_version"], policy["base_map_hash"] = expected
    policy["map_version"], policy["map_hash"] = refined_map["map_version"], digest
    return refined_map, ids, refined_evidence, policy, names, child_groups


def main() -> int:
    config = read_config()
    map_data = read_json(OUTPUT / "provinces.json")
    evidence = read_json(OUTPUT / "polities_1440.json")
    ids = decode_ids(OUTPUT / "provinces.png")
    terrain = np.asarray(Image.open(OUTPUT / "terrain.png"), dtype=np.uint8)
    rivers = np.asarray(Image.open(OUTPUT / "rivers.png"), dtype=np.uint8)
    names = read_json(OUTPUT / "names.en.json")
    scenario = read_json(SCENARIO_PATH)
    base_keys = {row["key"] for row in map_data["provinces"]}
    countries = {tag: read_json(COUNTRY_DIR / f"{tag}.json") for tag in scenario["countries"]}
    owners: dict[str, str] = {}
    for tag, country in countries.items():
        for key in country["start"]["provinces"]:
            if key in base_keys:
                if key in owners:
                    raise ValueError(f"base province {key} has multiple owners")
                owners[key] = tag
    land_keys = {row["key"] for row in map_data["provinces"] if row["kind"] == "land"}
    unowned_keys = set(scenario["unowned_provinces"])
    if not unowned_keys <= land_keys:
        raise ValueError("scenario has unknown or non-land unowned keys before refinement")
    if set(owners) & unowned_keys:
        raise ValueError("a base province is both owned and declared unowned")
    missing = sorted(land_keys - set(owners) - unowned_keys)
    if missing:
        raise ValueError(f"base land must be owned or explicitly unowned; missing {missing[:8]}")

    refined_map, ids, refined_evidence, policy, names, child_groups = refine(
        map_data, ids, terrain, rivers, evidence, owners, names, config
    )

    # Retain each baseline country assignment and append only children created here.
    children_by_owner: dict[str, list[str]] = {tag: [] for tag in countries}
    row_by_id = {row["id"]: row for row in refined_map["provinces"]}
    for parent_id, child_ids in child_groups.items():
        if len(child_ids) <= 1:
            continue
        tag = owners[row_by_id[parent_id]["key"]]
        children_by_owner[tag].extend(row_by_id[child]["key"] for child in child_ids[1:])
    for tag, country in countries.items():
        current = [key for key in country["start"]["provinces"] if key in base_keys]
        country["start"]["provinces"] = sorted(set(current + children_by_owner[tag]))

    frontier = policy["rules"].get("northern_russia_frontier", {})
    frontier_tags = set(frontier.get("owner_tags", []))
    province_by_key = {row["key"]: row for row in refined_map["provinces"]}
    newly_unowned = set()
    for tag, country in countries.items():
        if tag not in frontier_tags:
            continue
        retained = []
        for key in country["start"]["provinces"]:
            row = province_by_key[key]
            x, y = row["centroid"]
            longitude = -13.0 + (x + 0.5) / 32.0
            latitude = 72.0 - (y + 0.5) / 32.0
            if (latitude >= frontier.get("minimum_latitude", math.inf)
                    and longitude >= frontier.get("minimum_longitude", math.inf)):
                newly_unowned.add(key)
            else:
                retained.append(key)
        country["start"]["provinces"] = retained
    scenario["unowned_provinces"] = sorted(
        set(scenario["unowned_provinces"]) | newly_unowned
    )
    unowned_keys.update(newly_unowned)

    land_keys = {row["key"] for row in refined_map["provinces"] if row["kind"] == "land"}
    claimed = {key for country in countries.values() for key in country["start"]["provinces"]}
    if claimed | unowned_keys != land_keys or claimed & unowned_keys:
        raise ValueError(
            f"refinement must keep exact owned/unowned land partition: "
            f"missing={len(land_keys - claimed - unowned_keys)}, "
            f"unknown={len((claimed | unowned_keys) - land_keys)}, "
            f"overlap={len(claimed & unowned_keys)}"
        )

    for tag, country in countries.items():
        (COUNTRY_DIR / f"{tag}.json").write_bytes(json_bytes(country, pretty=True))

    rgb = np.stack(((ids >> 16) & 0xff, (ids >> 8) & 0xff, ids & 0xff), axis=-1).astype(np.uint8)
    Image.fromarray(rgb).save(OUTPUT / "provinces.png", compress_level=9)
    (OUTPUT / "provinces.json").write_bytes(json_bytes(refined_map, pretty=True))
    (OUTPUT / "polities_1440.json").write_bytes(json_bytes(refined_evidence, pretty=True))
    (OUTPUT / "names.en.json").write_bytes(json_bytes(names, pretty=True))
    (SCENARIO_PATH).write_bytes(json_bytes(scenario, pretty=True))
    OWNERSHIP_POLICY_PATH.write_bytes(json_bytes(policy, pretty=True))

    land_count = sum(row["kind"] == "land" for row in refined_map["provinces"])
    added = sum(len(children) - 1 for children in child_groups.values())
    if max(row["id"] for row in refined_map["provinces"]) >= EUROPE_ID_LIMIT:
        raise SystemExit(f"Europe ID reservation exceeded: {max(row['id'] for row in refined_map['provinces'])}")
    print(
        f"Europe map v{refined_map['map_version']}: {land_count} land provinces, "
        f"+{added} refined children, {len(scenario['unowned_provinces'])} unowned frontier cells, "
        f"{len(refined_map['provinces'])} total records; "
        f"map_hash={refined_map['map_hash']}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
