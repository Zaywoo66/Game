#!/usr/bin/env python3
"""Apply conservative steppe owner corrections from a local AoC2 audit CSV.

The CSV is a comparison report, not a source asset committed to the project.
Only four explicitly reviewed polity mappings are eligible. No province
geometry or AoC2 scenario data is copied into the repository.
"""

from __future__ import annotations

import argparse
from collections import Counter
import csv
from decimal import Decimal, InvalidOperation
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAP_PATH = ROOT / "data/map/europe/provinces.json"
SCENARIO_PATH = ROOT / "data/scenarios/1440.json"
COUNTRY_DIR = ROOT / "data/countries"
TAG_MAP = {
    "greh_h": "gth",
    "nago_h": "nog",
    "bukh_m": "uzb",
    "crkh_h": "cri",
}
MIN_CONFIDENCE = Decimal("0.75")
MIN_MATCHED_SAMPLES = 8
REQUIRED_COLUMNS = {
    "province_id", "province_key", "current_owner", "aoc2_tag",
    "confidence", "matched_samples", "total_samples", "other_tags",
}


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def plan_reassignments(map_data, countries, rows):
    """Return owner moves and review counts without mutating input content."""
    provinces = {row["key"]: row for row in map_data["provinces"]}
    owner_by_key = {}
    for tag, country in countries.items():
        for key in country["start"]["provinces"]:
            if key in owner_by_key:
                raise ValueError(f"province {key} has multiple current owners")
            owner_by_key[key] = tag

    capitals = {country["capital"] for country in countries.values()}
    seen = set()
    moves = []
    counts = Counter()
    for row in rows:
        tag = row["aoc2_tag"].strip()
        if tag not in TAG_MAP:
            counts["other_aoc2_tag"] += 1
            continue
        key = row["province_key"].strip()
        if key in seen:
            raise ValueError(f"duplicate review row for {key}")
        seen.add(key)
        province = provinces.get(key)
        if province is None or province["kind"] != "land":
            raise ValueError(f"review row references unknown/non-land province {key}")
        try:
            province_id = int(row["province_id"])
            matched = int(row["matched_samples"])
            total = int(row["total_samples"])
            confidence = Decimal(row["confidence"])
        except (ValueError, InvalidOperation) as exc:
            raise ValueError(f"invalid numeric audit evidence for {key}") from exc
        if province_id != province["id"]:
            raise ValueError(f"province ID mismatch for {key}")
        if total <= 0 or matched < 0 or matched > total or not confidence.is_finite():
            raise ValueError(f"invalid sample count or confidence for {key}")
        if not Decimal(0) <= confidence <= Decimal(1):
            raise ValueError(f"confidence outside 0..1 for {key}")
        current_owner = owner_by_key.get(key)
        if current_owner is None or row["current_owner"].strip() != current_owner:
            raise ValueError(f"stale/unmatched current owner for {key}")
        if confidence < MIN_CONFIDENCE or matched < MIN_MATCHED_SAMPLES:
            counts["low_confidence"] += 1
            continue
        target = TAG_MAP[tag]
        if target not in countries:
            raise ValueError(f"target country {target} is absent")
        if target == current_owner:
            counts["already_correct"] += 1
            continue
        if key in capitals:
            counts["capital_protected"] += 1
            continue
        moves.append((key, current_owner, target))
        counts["move_to_" + target] += 1

    counts["eligible_moves"] = len(moves)
    return moves, counts


def apply_moves(countries, moves):
    """Update only affected country records after all validations succeed."""
    affected = set()
    for key, source, target in moves:
        countries[source]["start"]["provinces"].remove(key)
        countries[target]["start"]["provinces"].append(key)
        affected.update((source, target))
    for tag in affected:
        countries[tag]["start"]["provinces"].sort()
    for tag, country in countries.items():
        if country["capital"] not in country["start"]["provinces"]:
            raise ValueError(f"{tag} lost its capital")
    return affected


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv", type=Path, help="local AoC2 comparison CSV")
    parser.add_argument("--apply", action="store_true", help="write reviewed owner moves")
    parser.add_argument("--show-moves", action="store_true", help="print each proposed owner move")
    args = parser.parse_args()

    with args.csv.open(newline="", encoding="utf-8-sig") as stream:
        reader = csv.DictReader(stream)
        if not reader.fieldnames or not REQUIRED_COLUMNS <= set(reader.fieldnames):
            raise SystemExit(f"CSV needs columns: {', '.join(sorted(REQUIRED_COLUMNS))}")
        rows = list(reader)
    map_data = read_json(MAP_PATH)
    scenario = read_json(SCENARIO_PATH)
    countries = {
        tag: read_json(COUNTRY_DIR / f"{tag}.json")
        for tag in scenario["countries"]
    }
    moves, counts = plan_reassignments(map_data, countries, rows)
    affected = apply_moves(countries, moves)
    for label, count in sorted(counts.items()):
        print(f"{label}: {count}")
    print(f"affected countries: {', '.join(sorted(affected)) or '(none)'}")
    if args.show_moves:
        for key, source, target in moves:
            print(f"{key}: {source} -> {target}")
    if args.apply:
        for tag in sorted(affected):
            path = COUNTRY_DIR / f"{tag}.json"
            path.write_text(
                json.dumps(countries[tag], ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
        print("applied reviewed moves")
    else:
        print("dry run; pass --apply to write country files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
