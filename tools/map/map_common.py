"""Shared map file conventions."""

from hashlib import sha256
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
REGION = "western_europe"
INPUTS = Path(__file__).resolve().parent / "inputs" / REGION
OUTPUT = ROOT / "data" / "map" / REGION
BOUNDS = (-13.0, 36.0, 12.0, 59.25)
WIDTH, HEIGHT = 768, 1024
ELEVATION_BIAS = 32768
SCHEMA_VERSION = 2
EDGE_ORDER = ("normal", "river", "strait", "port")
PROVINCE_FIELDS = (
    "id", "key", "kind", "terrain", "centroid", "neighbors", "ports", "development"
)
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


def json_bytes(value, *, pretty=False):
    if pretty:
        return (json.dumps(value, ensure_ascii=True, indent=2) + "\n").encode("ascii")
    return json.dumps(value, ensure_ascii=True, separators=(",", ":")).encode("ascii")


def map_hash(provinces, schema_version=SCHEMA_VERSION):
    canonical = []
    for record in sorted(provinces, key=lambda p: p["id"]):
        province = {field: record[field] for field in PROVINCE_FIELDS}
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
    return sha256(json_bytes(canonical)).hexdigest()
