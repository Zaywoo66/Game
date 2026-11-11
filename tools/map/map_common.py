"""Shared map file conventions."""

from hashlib import sha256
import json
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
REGION = "western_europe"
INPUTS = Path(__file__).resolve().parent / "inputs" / REGION
OUTPUT = ROOT / "data" / "map" / REGION
BOUNDS = (-13.0, 36.0, 12.0, 59.25)
PIXELS_PER_DEGREE = 32
PROJECTION = {"pixels_per_degree": PIXELS_PER_DEGREE, "origin": [5344, 984]}
WIDTH, HEIGHT = 800, 744
EUROPE_ID_RANGE = range(1, 2000)
ELEVATION_BIAS = 32768
SCHEMA_VERSION = 3
EDGE_ORDER = ("normal", "river", "strait", "port")
PROVINCE_FIELDS = (
    "id", "key", "kind", "terrain", "centroid", "neighbors", "ports", "development"
)
PROVINCE_V3_FIELDS = (*PROVINCE_FIELDS, "name_key", "area")
TERRAINS = ("sea", "plains", "hills", "mountains", "lake")


def unique_object(pairs):
    result = {}
    for key, value in pairs:
        if key in result:
            raise ValueError(f"duplicate JSON field {key!r}")
        result[key] = value
    return result


def read_json(path):
    return json.loads(path.read_bytes(), object_pairs_hook=unique_object)


class UniqueYamlLoader(yaml.SafeLoader):
    def construct_mapping(self, node, deep=False):
        self.flatten_mapping(node)
        return unique_object(self.construct_pairs(node, deep=deep))


def read_yaml(path):
    return yaml.load(path.read_bytes(), Loader=UniqueYamlLoader)


def yaml_bytes(value):
    return yaml.safe_dump(value, sort_keys=False).encode("utf-8")


def json_bytes(value, *, pretty=False):
    if pretty:
        return (json.dumps(value, ensure_ascii=True, indent=2) + "\n").encode("ascii")
    return json.dumps(value, ensure_ascii=True, separators=(",", ":")).encode("ascii")


def map_hash(provinces, schema_version=SCHEMA_VERSION, projection=None):
    canonical = []
    for record in sorted(provinces, key=lambda p: p["id"]):
        fields = PROVINCE_V3_FIELDS if schema_version >= 3 else PROVINCE_FIELDS
        province = {field: record[field] for field in fields}
        province["neighbors"] = [
            {"id": edge["id"], "edge": edge["edge"], **(
                {"cost": edge["cost"]} if schema_version >= 2 else {}
            )}
            for edge in sorted(
                record["neighbors"],
                key=lambda e: (e["id"], EDGE_ORDER.index(e["edge"])),
            )
        ]
        province["ports"] = sorted(record["ports"])
        canonical.append(province)
    if schema_version >= 3:
        canonical = {
            "projection": {field: projection[field] for field in PROJECTION},
            "provinces": canonical,
        }
    return sha256(json_bytes(canonical)).hexdigest()
