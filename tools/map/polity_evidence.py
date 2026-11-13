"""Reproducible source-polygon overlap evidence; never assigns game ownership."""

from collections import Counter
from hashlib import sha256

import numpy as np

from historical_areas import HISTORY_SOURCE
from map_common import INPUTS

SOURCE_FILE = INPUTS / "historical_1440.geojson"
COARSE_GROUPS = {"Holy Roman Empire Minor States", "Kalmar Union", "(Kalmar Union)"}


def source_digest():
    # Git's canonical text is LF; Windows checkouts may convert it to CRLF.
    return sha256(SOURCE_FILE.read_bytes().replace(b"\r\n", b"\n")).hexdigest()


def build_evidence(ids, history, features, provinces, map_version, map_hash):
    assignments = []
    for province in provinces:
        if province["kind"] != "land":
            continue
        pixels = history[ids == province["id"]]
        counts = Counter()
        for feature_id, amount in enumerate(np.bincount(pixels)):
            if feature_id and amount:
                counts[features[feature_id - 1]["properties"]["name"]] += int(amount)
        name = min(counts, key=lambda candidate: (-counts[candidate], candidate)) if counts else None
        assignments.append({
            "province_id": province["id"],
            "province_key": province["key"],
            "polity_name": name,
            "resolution": "unmatched" if name is None else "coarse_group" if name in COARSE_GROUPS else "polygon",
            "overlap_pixels": counts[name] if name is not None else 0,
            "province_pixels": len(pixels),
        })
    return {
        "schema_version": 1,
        "map_version": map_version,
        "map_hash": map_hash,
        "reference_year": 1440,
        "source": {
            "name": "Cliopatria",
            "release": "v0.2.1",
            "url": HISTORY_SOURCE,
            "license": "CC BY 4.0",
            "file": "tools/map/inputs/western_europe/historical_1440.geojson",
            "sha256": source_digest(),
        },
        "method": "largest rasterized source-polygon pixel overlap; documented Bavaria-to-Burgundian-Netherlands correction; lexical tie break",
        "assignments": assignments,
    }
