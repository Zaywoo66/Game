"""Validate the Western Europe M2 raster and province graph."""

import argparse
from collections import Counter
import re
import struct
from pathlib import Path

import numpy as np
from PIL import Image

from map_common import (
    EDGE_ORDER, ELEVATION_BIAS, HEIGHT, INPUTS, OUTPUT, PROVINCE_FIELDS, REGION, TERRAINS, WIDTH,
    map_hash, read_json,
)
from topology import raster_contacts, regular_edge, strait_pairs

KEY_PATTERN = re.compile(r"[a-z][a-z0-9]*(?:_[a-z0-9]+)*")
MAX_RGB_ID = (1 << 24) - 1
KINDS = {"land", "sea", "lake"}
TERRAIN_INDEXES = {"land": (1, 2, 3), "sea": (0,), "lake": (4,)}


def require(condition, message):
    if not condition:
        raise ValueError(message)


def integer(value, minimum, maximum):
    return type(value) is int and minimum <= value <= maximum


def read_png(directory, filename, *, depth, color_type):
    path = directory / filename
    with path.open("rb") as source:
        header = source.read(29)
    require(
        len(header) == 29 and header[:16] == b"\x89PNG\r\n\x1a\n\x00\x00\x00\x0dIHDR",
        f"{filename}: invalid PNG header",
    )
    width, height, bits, color, compression, filters, interlace = struct.unpack(">IIBBBBB", header[16:])
    require((width, height) == (WIDTH, HEIGHT), f"{filename}: incorrect raster dimensions")
    require(
        (bits, color, compression, filters, interlace) == (depth, color_type, 0, 0, 0),
        f"{filename}: incorrect PNG format; expected depth={depth}, color type={color_type}, no interlace",
    )
    with Image.open(path) as image:
        return np.array(image)


def parse_provinces(path, config):
    metadata = read_json(path)
    require(isinstance(metadata, dict) and set(metadata) == {
        "schema_version", "map_version", "map_hash", "provinces",
    }, "provinces.json: invalid metadata fields")
    require(integer(metadata["schema_version"], 1, 1), "provinces.json: unsupported schema_version")
    require(
        integer(metadata["map_version"], 1, (1 << 32) - 1)
        and metadata["map_version"] == config["map_version"],
        "provinces.json: invalid map_version",
    )
    entries = metadata["provinces"]
    require(isinstance(entries, list) and 30 <= len(entries) <= 50, "expected 30 to 50 province records")
    ids, keys = {}, set()
    for entry in entries:
        require(isinstance(entry, dict) and set(entry) == set(PROVINCE_FIELDS), "invalid province record fields")
        province, key = entry["id"], entry["key"]
        require(integer(province, 1, MAX_RGB_ID) and province not in ids, f"invalid or duplicate RGB24 province ID {province}")
        require(isinstance(key, str) and KEY_PATTERN.fullmatch(key) and key not in keys, f"invalid or duplicate province key {key!r}")
        ids[province] = entry
        keys.add(key)
        require(isinstance(entry["kind"], str) and entry["kind"] in KINDS, f"{key}: invalid kind")
        require(isinstance(entry["terrain"], str) and entry["terrain"] in TERRAINS, f"{key}: invalid terrain")
        require(integer(entry["development"], 0, (1 << 32) - 1), f"{key}: invalid development")
        center = entry["centroid"]
        require(
            isinstance(center, list) and len(center) == 2
            and integer(center[0], 0, WIDTH - 1) and integer(center[1], 0, HEIGHT - 1),
            f"{key}: centroid is outside the map",
        )
        require(isinstance(entry["neighbors"], list), f"{key}: invalid neighbors list")
        require(isinstance(entry["ports"], list), f"{key}: invalid ports list")
    expected = {key: kind for kind in KINDS for key in config[kind]}
    require({p["key"]: p["kind"] for p in entries} == expected, "province keys/kinds differ from region overrides")

    for province, entry in ids.items():
        key = entry["key"]
        edges = set()
        for edge in entry["neighbors"]:
            require(isinstance(edge, dict) and set(edge) == {"id", "edge"}, f"{key}: invalid edge fields")
            target, kind = edge["id"], edge["edge"]
            require(
                integer(target, 1, MAX_RGB_ID) and target in ids and target != province
                and isinstance(kind, str) and kind in EDGE_ORDER,
                f"{key}: invalid neighbor {edge}",
            )
            require((target, kind) not in edges, f"{key}: duplicate edge {edge}")
            edges.add((target, kind))
        ports = entry["ports"]
        for target in ports:
            require(
                integer(target, 1, MAX_RGB_ID) and target in ids and ids[target]["kind"] == "sea",
                f"{key}: invalid sea port {target}",
            )
        require(len(set(ports)) == len(ports), f"{key}: duplicate port")
    require(metadata["map_hash"] == map_hash(entries), "map_hash does not match canonical province records")
    return ids


def require_land_reachable(records, neighbors):
    land = {province for province, entry in records.items() if entry["kind"] == "land"}
    seen = {min(land)}
    work = list(seen)
    while work:
        province = work.pop()
        for target, kind in neighbors[province]:
            if target in land and kind in ("normal", "river", "strait") and target not in seen:
                seen.add(target)
                work.append(target)
    missing = sorted(records[province]["key"] for province in land - seen)
    require(not missing, f"unreachable land provinces: {', '.join(missing)}")


def validate_edges(ids, river, land, sea, records, config):
    contacts, river_contacts = raster_contacts(ids, river)
    kinds = {province: entry["kind"] for province, entry in records.items()}
    actual = {
        province: {(edge["id"], edge["edge"]) for edge in entry["neighbors"]}
        for province, entry in records.items()
    }
    for province, edges in actual.items():
        for target, kind in sorted(edges):
            require((province, kind) in actual[target], f"missing reverse {kind} edge {target} -> {province}")

    expected = {province: set() for province in records}

    def add(a, b, kind):
        expected[a].add((b, kind))
        expected[b].add((a, kind))

    for pair in contacts:
        add(*pair, regular_edge(pair, contacts, river_contacts, kinds))
    for pair in strait_pairs(ids, land, sea, config["straits"]):
        require(pair not in contacts, f"strait endpoints {pair} already touch")
        add(*pair, "strait")
    expected_port_keys = set(config["port_provinces"])
    for province, entry in records.items():
        key = entry["key"]
        if key not in expected_port_keys:
            require(not entry["ports"], f"{key}: port is absent from region overrides")
            continue
        require(entry["kind"] == "land", f"{key}: port override does not name a land province")
        candidates = [
            (sea_id, contacts[tuple(sorted((province, sea_id)))])
            for sea_id, edge in expected[province]
            if kinds[sea_id] == "sea" and edge == "normal"
        ]
        require(candidates, f"{key}: port override does not border the sea")
        expected_sea, _ = min(candidates, key=lambda item: (-item[1], item[0]))
        require(entry["ports"] == [expected_sea], f"{key}: expected port to sea province {expected_sea}")
        add(province, expected_sea, "port")

    for province, edges in actual.items():
        if edges != expected[province]:
            missing = sorted(expected[province] - edges)
            extra = sorted(edges - expected[province])
            raise ValueError(f"province {province}: raster/override adjacency differs (missing={missing}, extra={extra})")
    require_land_reachable(records, actual)


def validate_directory(directory):
    config = read_json(INPUTS / "overrides.json")
    records = parse_provinces(directory / "provinces.json", config)
    rgb = read_png(directory, "provinces.png", depth=8, color_type=2).astype(np.uint32)
    ids = (rgb[..., 0] << 16) | (rgb[..., 1] << 8) | rgb[..., 2]
    heights = read_png(directory, "heightmap.png", depth=16, color_type=0)
    terrain = read_png(directory, "terrain.png", depth=8, color_type=0)
    rivers = read_png(directory, "rivers.png", depth=8, color_type=0)
    require(heights.min() < ELEVATION_BIAS < heights.max(), "heightmap must contain bathymetry and above-sea-level elevation")
    require(np.all(terrain < len(TERRAINS)), "terrain.png: unknown terrain index")
    require(np.all((rivers == 0) | (rivers == 255)), "rivers.png: expected a binary 0/255 mask")

    unique, counts = np.unique(ids, return_counts=True)
    pixels = dict(zip(map(int, unique), map(int, counts), strict=True))
    require(0 not in pixels, "provinces.png: orphan pixels use reserved ID 0")
    require(set(pixels) == set(records), f"raster IDs differ from metadata: {sorted(set(pixels) ^ set(records))}")

    masks = {kind: np.zeros(ids.shape, dtype=bool) for kind in KINDS}
    for province, entry in records.items():
        key = entry["key"]
        require(pixels[province] > 1, f"{key}: one-pixel province")
        x, y = entry["centroid"]
        require(ids[y, x] == province, f"{key}: centroid does not select its own province")
        mask = ids == province
        index = terrain[mask]
        require(np.all(np.isin(index, TERRAIN_INDEXES[entry["kind"]])), f"{key}: terrain incompatible with province kind")
        require(TERRAINS[np.bincount(index).argmax()] == entry["terrain"], f"{key}: metadata terrain differs from raster majority")
        masks[entry["kind"]] |= mask
    require(not np.any((rivers != 0) & ~masks["land"]), "rivers.png: river pixels must be on land")
    validate_edges(ids, rivers != 0, masks["land"], masks["sea"], records, config)
    return Counter(record["kind"] for record in records.values())


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--region", choices=(REGION,), required=True)
    parser.add_argument("--map-dir", type=Path, default=OUTPUT, help="Validate another copy of the region")
    args = parser.parse_args()
    try:
        kinds = validate_directory(args.map_dir)
    except (OSError, ValueError) as error:
        parser.exit(1, f"map validation failed: {error}\n")
    print(f"{args.region}: valid {WIDTH}x{HEIGHT} map, {sum(kinds.values())} provinces {dict(kinds)}")


if __name__ == "__main__":
    main()
