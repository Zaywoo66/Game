#!/usr/bin/env python3
"""Crosswalk coarse HRE map cells to the ETH c.1400 territory polygons.

This produces research suggestions only. It does not change campaign ownership,
province geometry, or historical source assignments.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from collections import Counter
from pathlib import Path

import numpy as np
import shapefile
from PIL import Image
from rasterio.features import rasterize
from rasterio.transform import from_bounds

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / "data/map/europe"
SOURCE = ROOT / "tools/map/inputs/hre_eth/territories_auto_semi"
OUTPUT = ROOT / "tools/map/inputs/hre_eth/hre_overlay_1440.json"
BOUNDS = (-13.0, 36.0, 68.0, 72.0)
MIN_OVERLAP = 0.50
MIN_LEAD = 0.15


def sha256(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as stream:
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return digest.hexdigest()


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=OUTPUT)
    args = parser.parse_args()

    provinces_doc = json.loads((DATA / "provinces.json").read_text(encoding="utf-8"))
    assignments_doc = json.loads(
        (DATA / "polities_1440.json").read_text(encoding="utf-8")
    )
    provinces_by_id = {row["id"]: row for row in provinces_doc["provinces"]}
    hre_cells = {
        int(row["province_id"]): row
        for row in assignments_doc["assignments"]
        if row["polity_name"] == "Holy Roman Empire Minor States"
        and row["resolution"] == "coarse_group"
    }
    if len(hre_cells) != 74:
        raise SystemExit(f"expected 74 coarse HRE cells, found {len(hre_cells)}")

    rgb = np.asarray(Image.open(DATA / "provinces.png").convert("RGB"), dtype=np.uint32)
    province_ids = (rgb[:, :, 0] << 16) | (rgb[:, :, 1] << 8) | rgb[:, :, 2]
    transform = from_bounds(*BOUNDS, rgb.shape[1], rgb.shape[0])

    reader = shapefile.Reader(str(SOURCE), encoding="latin1")
    fields = [field[0] for field in reader.fields[1:]]
    territories = []
    for shape, record in zip(reader.shapes(), reader.records(), strict=True):
        attrs = dict(zip(fields, record, strict=True))
        old_name = attrs["old_name"].strip()
        territories.append(
            {
                "source_index": len(territories),
                "name": old_name.split("#", 1)[0],
                "source_name": old_name,
                "modern_name": attrs["modern_nam"].strip(),
                "territory_type": attrs["type_of_ru"].strip(),
                "geometry": shape.__geo_interface__,
            }
        )

    raster_shapes = [
        (territory["geometry"], territory["source_index"] + 1)
        for territory in territories
    ]
    territory_ids = rasterize(
        raster_shapes,
        out_shape=province_ids.shape,
        transform=transform,
        fill=0,
        dtype="uint16",
        all_touched=False,
    )

    output_rows = []
    assigned = Counter()
    territory_types: dict[str, str] = {}
    for province_id, source_row in sorted(hre_cells.items()):
        cell_mask = province_ids == province_id
        denominator = int(cell_mask.sum())
        counts = Counter(int(value) for value in territory_ids[cell_mask] if value)
        ranked = sorted(counts.items(), key=lambda item: (-item[1], item[0]))
        top_id, top_pixels = ranked[0] if ranked else (None, 0)
        second_pixels = ranked[1][1] if len(ranked) > 1 else 0
        coverage = sum(counts.values()) / denominator if denominator else 0.0
        top_share = top_pixels / denominator if denominator else 0.0
        lead = (top_pixels - second_pixels) / denominator if denominator else 0.0
        confident = top_id is not None and top_share >= MIN_OVERLAP and lead >= MIN_LEAD
        territory = territories[top_id - 1] if top_id is not None else None
        next_territory = territories[ranked[1][0] - 1] if len(ranked) > 1 else None
        if confident and territory:
            assigned[territory["name"]] += 1
            territory_types[territory["name"]] = territory["territory_type"]
        output_rows.append(
            {
                "province_id": province_id,
                "province_key": provinces_by_id[province_id]["key"],
                "territory": territory["name"] if confident and territory else None,
                "territory_source_name": territory["source_name"] if confident and territory else None,
                "modern_name": territory["modern_name"] if confident and territory else None,
                "source_territory_type": territory["territory_type"] if confident and territory else None,
                "overlap_share": round(top_share, 6),
                "source_coverage": round(coverage, 6),
                "runner_up": next_territory["name"] if next_territory else None,
                "runner_up_share": round(second_pixels / denominator, 6) if denominator else 0.0,
                "uncertainty": (
                    "assigned_by_overlap; c.1400 source is not verified for 1440"
                    if confident
                    else "unassigned: no polygon or overlap below threshold / ambiguous"
                ),
                "source_resolution": source_row["resolution"],
            }
        )

    result = {
        "schema_version": 1,
        "map_version": provinces_doc["map_version"],
        "map_hash": provinces_doc["map_hash"],
        "reference_year": 1400,
        "target_start_date": "1440-01-01",
        "method": {
            "name": "province-raster overlap with ETH automatic semi territory polygons",
            "raster_bounds_lon_lat": list(BOUNDS),
            "minimum_cell_share": MIN_OVERLAP,
            "minimum_lead_over_runner_up": MIN_LEAD,
            "ambiguity_policy": "leave cell unassigned when either threshold is missed",
            "warning": "The static c.1400 ETH dataset is an approximate reference, not verified 1440 sovereignty.",
        },
        "source": {
            "title": "Static spatial data on territories of the Holy Roman Empire",
            "author": "Ramona Roller",
            "doi": "10.3929/ethz-b-000472585",
            "url": "https://www.research-collection.ethz.ch/entities/researchdata/2f7da3dd-d3ef-42f9-9a46-6504511ca522",
            "license": "CC BY-NC-SA 4.0",
            "snapshot_sha256": {
                path.suffix: sha256(path)
                for path in sorted(SOURCE.parent.glob(SOURCE.name + ".*"))
            },
        },
        "assigned_cell_count": sum(1 for row in output_rows if row["territory"]),
        "unassigned_cell_count": sum(1 for row in output_rows if not row["territory"]),
        "territory_cell_counts": dict(sorted(assigned.items())),
        "roster_recommendations": [
            {
                "candidate_name": name,
                "source_territory_type": territory_types[name],
                "province_keys": [row["province_key"] for row in output_rows if row["territory"] == name],
                "review_note": "candidate independent polity only; verify 1440 status, borders, and ruler before creating country content",
            }
            for name in sorted(assigned)
        ],
        "assignments": output_rows,
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(result, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")
    print(
        f"Wrote {args.output.relative_to(ROOT)}: {result['assigned_cell_count']} assigned, "
        f"{result['unassigned_cell_count']} unassigned; {len(territories)} source territories"
    )


if __name__ == "__main__":
    main()
