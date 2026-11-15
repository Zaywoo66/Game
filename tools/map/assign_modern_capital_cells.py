"""Carve the Lebanese capital from a coarse neighboring raster cell."""

from __future__ import annotations

import json

import numpy as np
from PIL import Image
from rasterio.features import rasterize
from rasterio.transform import from_bounds

import build_modern_scenario as modern
from map_common import ROOT, json_bytes, map_hash, read_json, read_yaml, yaml_bytes
from topology import raster_contacts, regular_edge


OUT = ROOT / "data/map/europe"
SCENARIO = ROOT / "data/scenarios/1440.json"
KEY = "beirut"
COUNTRY_CODE = "LBN"


def main() -> None:
    metadata = read_json(OUT / "provinces.json")
    if metadata["map_version"] != 8:
        raise ValueError("expected the unpatched combined map v8")
    ids_rgb = np.asarray(Image.open(OUT / "provinces.png").convert("RGB"), dtype=np.uint32)
    ids = (ids_rgb[..., 0] << 16) | (ids_rgb[..., 1] << 8) | ids_rgb[..., 2]
    height, width = ids.shape
    modern.BOUNDS = (-18.0, 10.0, 68.0, 72.0)
    groups, capitals, _ = modern.read_groups()
    group = next(item for item in groups if item["code"] == COUNTRY_CODE)
    lon, lat = capitals[COUNTRY_CODE]
    land = rasterize(((group["geometry"], 1),), out_shape=ids.shape,
                     transform=from_bounds(*modern.BOUNDS, width, height),
                     dtype=np.uint8).astype(bool)
    carved = land & (ids != 0)
    if int(carved.sum()) < 16:
        raise ValueError(f"{KEY}: too few source land pixels to make a province")

    rows = {int(row["id"]): dict(row) for row in metadata["provinces"]}
    if any(row["key"] == KEY for row in rows.values()):
        raise ValueError(f"province {KEY} already exists")
    new_id = max(rows) + 1
    affected = set(map(int, np.unique(ids[carved])))
    ids[carved] = new_id
    rows[new_id] = {
        "id": new_id, "key": KEY, "kind": "land", "terrain": "plains",
        "centroid": [int(np.mean(np.where(ids == new_id)[1])), int(np.mean(np.where(ids == new_id)[0]))],
        "neighbors": [], "ports": [], "development": 1,
        "name_key": f"province_{KEY}", "area": "africa_middle_east_unclassified",
    }
    for province in affected:
        y, x = np.where(ids == province)
        if not len(x):
            raise ValueError(f"carving {KEY} emptied province {province}")
        rows[province]["centroid"] = [int(np.mean(x)), int(np.mean(y))]

    costs = read_yaml(ROOT / "tools/map/inputs/western_europe/overrides.yaml")["movement_costs"]
    kinds = {province: row["kind"] for province, row in rows.items()}
    contacts, river_contacts = raster_contacts(ids, np.asarray(Image.open(OUT / "rivers.png")) != 0)
    edges = {province: {(edge["id"], edge["edge"]): edge["cost"]
                        for edge in row["neighbors"] if edge["edge"] in ("port", "strait")}
             for province, row in rows.items()}
    for pair in contacts:
        edge = regular_edge(pair, contacts, river_contacts, kinds)
        edges[pair[0]][(pair[1], edge)] = costs[edge]
        edges[pair[1]][(pair[0], edge)] = costs[edge]
    for province, row in rows.items():
        row["neighbors"] = [
            {"id": other, "edge": edge, "cost": cost}
            for (other, edge), cost in sorted(edges[province].items(), key=lambda item: (item[0][0], item[0][1]))
        ]

    names = read_json(OUT / "names.en.json")
    names[f"province_{KEY}"] = "Beirut"
    provinces = [rows[province] for province in sorted(rows)]
    metadata["map_version"] = 9
    metadata["map_hash"] = map_hash(provinces, projection=metadata["projection"])
    metadata["provinces"] = provinces
    Image.fromarray(np.stack((ids >> 16, ids >> 8, ids), axis=-1).astype(np.uint8)).save(
        OUT / "provinces.png", compress_level=9
    )
    (OUT / "provinces.json").write_bytes(json_bytes(metadata, pretty=True))
    (OUT / "names.en.json").write_bytes(json_bytes(names, pretty=True))

    evidence = read_json(OUT / "polities_1440.json")
    _, counts = np.unique(ids, return_counts=True)
    id_counts = dict(zip(map(int, np.unique(ids)), map(int, counts), strict=True))
    for assignment in evidence["assignments"]:
        province = assignment["province_id"]
        assignment["province_pixels"] = id_counts[province]
        assignment["overlap_pixels"] = min(assignment["overlap_pixels"], id_counts[province])
    evidence["assignments"].append({
        "province_id": new_id, "province_key": KEY, "polity_name": None,
        "resolution": "unmatched", "overlap_pixels": 0,
        "province_pixels": id_counts[new_id],
    })
    evidence["map_version"], evidence["map_hash"] = 9, metadata["map_hash"]
    (OUT / "polities_1440.json").write_bytes(json_bytes(evidence, pretty=True))
    ownership = read_json(OUT / "ownership_1440.json")
    ownership["map_version"], ownership["map_hash"] = 9, metadata["map_hash"]
    (OUT / "ownership_1440.json").write_bytes(json_bytes(ownership, pretty=True))
    scenario = read_json(SCENARIO)
    scenario["unowned_provinces"] = sorted(set(scenario["unowned_provinces"]) | {KEY})
    SCENARIO.write_bytes(json_bytes(scenario, pretty=True))
    print(f"added {KEY} province {new_id} ({id_counts[new_id]} pixels); map v9 {metadata['map_hash']}")


if __name__ == "__main__":
    main()
