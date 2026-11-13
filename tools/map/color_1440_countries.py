#!/usr/bin/env python3
"""Assign a muted, distinct country palette using the 1440 land adjacency graph."""

import argparse
import hashlib
import json
import math
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAP_PATH = ROOT / "data/map/europe/provinces.json"
SCENARIO_PATH = ROOT / "data/scenarios/1440.json"
COUNTRY_DIR = ROOT / "data/countries"


def _hsl_color(hue: int, saturation: float, lightness: float) -> tuple[int, int, int]:
    chroma = (1.0 - abs(2.0 * lightness - 1.0)) * saturation
    segment = hue / 60.0
    second = chroma * (1.0 - abs(segment % 2.0 - 1.0))
    rgb = (
        (chroma, second, 0.0) if segment < 1 else
        (second, chroma, 0.0) if segment < 2 else
        (0.0, chroma, second) if segment < 3 else
        (0.0, second, chroma) if segment < 4 else
        (second, 0.0, chroma) if segment < 5 else
        (chroma, 0.0, second)
    )
    offset = lightness - chroma / 2.0
    return tuple(round((component + offset) * 255) for component in rgb)


PALETTE = tuple(_hsl_color(index * 30, 0.34, 0.66) for index in range(12))


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def assign_colors(scenario, countries, map_data):
    """Return a copy with unique colors chosen to separate adjacent countries."""
    province_by_key = {province["key"]: province for province in map_data["provinces"]}
    province_by_id = {province["id"]: province for province in map_data["provinces"]}
    owner_by_id = {}
    for tag, country in countries.items():
        for key in country["start"]["provinces"]:
            province = province_by_key.get(key)
            if province is None:
                continue
            owner_by_id[province["id"]] = tag

    neighbors = {tag: set() for tag in scenario["countries"]}
    for province_id, tag in owner_by_id.items():
        for edge in province_by_id[province_id]["neighbors"]:
            other = owner_by_id.get(edge["id"])
            if other is not None and other != tag:
                neighbors[tag].add(other)
                neighbors[other].add(tag)

    palette = list(PALETTE)
    extra_levels = ((0.34, 0.74), (0.42, 0.57), (0.24, 0.70), (0.46, 0.64))
    level = 0
    while len(palette) < len(neighbors):
        saturation, lightness = extra_levels[level % len(extra_levels)]
        hue_offset = 15 * (level + 1)
        palette.extend(
            _hsl_color((index * 30 + hue_offset) % 360, saturation, lightness)
            for index in range(12)
        )
        level += 1

    def stable_offset(tag: str) -> int:
        return int.from_bytes(hashlib.sha256(tag.encode("utf-8")).digest()[:4], "big") % len(palette)

    assigned = {}
    available = set(range(len(palette)))
    order = sorted(neighbors, key=lambda tag: (-len(neighbors[tag]), tag))
    for tag in order:
        adjacent = [assigned[neighbor] for neighbor in neighbors[tag] if neighbor in assigned]
        choices = sorted(available, key=lambda index: ((index - stable_offset(tag)) % len(palette)))

        def distance(left, right):
            return math.sqrt(sum((a - b) ** 2 for a, b in zip(left, right)))

        index = max(
            choices,
            key=lambda choice: min(
                (distance(palette[choice], palette[neighbor]) for neighbor in adjacent),
                default=float("inf"),
            ),
        )
        assigned[tag] = index
        available.remove(index)

    updated = json.loads(json.dumps(countries))
    for tag, index in assigned.items():
        updated[tag]["color"] = list(palette[index])
    return updated, assigned


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true", help="write updated country colors")
    args = parser.parse_args()
    scenario = read_json(SCENARIO_PATH)
    countries = {tag: read_json(COUNTRY_DIR / f"{tag}.json") for tag in scenario["countries"]}
    map_data = read_json(MAP_PATH)
    updated, indices = assign_colors(scenario, countries, map_data)
    for tag in sorted(updated):
        print(f"{tag}: {updated[tag]['color']}")
    if args.apply:
        for tag, country in updated.items():
            write_json(COUNTRY_DIR / f"{tag}.json", country)
    else:
        print("dry run; pass --apply to write country colors")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
