"""Compare local AoC2 1440 owners with our province samples.

This is a local, read-only diagnostic. AoC2's proprietary files are never
copied to the repository or included in the output. Install optional tooling
with ``python -m pip install javaobj-py3 shapely pillow`` before running.
"""

from __future__ import annotations

import argparse
import csv
import json
import math
import re
from collections import Counter
from pathlib import Path


GAME_ROOT = Path(r"C:\Program Files (x86)\Age of Civilizations II")
CITY_RE = re.compile(r'Name:\s*"([^"\r\n]+)"\s*,\s*x:\s*(-?\d+)\s*,\s*y:\s*(-?\d+)')


def mercator(latitude: float) -> float:
    return math.log(math.tan(math.pi / 4 + math.radians(latitude) / 2))


def fit_line(pairs: list[tuple[float, float]]) -> tuple[float, float]:
    x_mean = sum(x for x, _ in pairs) / len(pairs)
    y_mean = sum(y for _, y in pairs) / len(pairs)
    denominator = sum((x - x_mean) ** 2 for x, _ in pairs)
    if denominator == 0:
        raise ValueError("city controls have no coordinate spread")
    slope = sum((x - x_mean) * (y - y_mean) for x, y in pairs) / denominator
    return y_mean - slope * x_mean, slope


def calibrate(game_root: Path, geojson_path: Path) -> tuple[float, float, float, float, int, float]:
    features = json.loads(geojson_path.read_text(encoding="utf-8"))["features"]
    places: dict[str, list[tuple[float, float]]] = {}
    for feature in features:
        name = feature["properties"].get("name")
        lon, lat = feature["geometry"]["coordinates"]
        if name and -13 <= lon <= 68 and 36 <= lat <= 72:
            places.setdefault(name, []).append((lon, lat))
    cities: dict[str, list[tuple[int, int]]] = {}
    for path in (game_root / "map/Earth/cities").glob("cities*.json"):
        for name, x, y in CITY_RE.findall(path.read_text(encoding="utf-8")):
            cities.setdefault(name, []).append((int(x), int(y)))

    # Initial projection is only a disambiguator for names reused worldwide.
    controls = []
    for name, locations in places.items():
        if len(locations) != 1 or name not in cities:
            continue
        lon, lat = locations[0]
        approximate_x = 2088 + 12.16 * lon
        approximate_y = 1593 - 673 * mercator(lat)
        x, y = min(cities[name], key=lambda p: math.hypot(p[0] - approximate_x, p[1] - approximate_y))
        if math.hypot(x - approximate_x, y - approximate_y) <= 30:
            controls.append((lon, mercator(lat), x, y))
    if len(controls) < 8:
        raise ValueError(f"only {len(controls)} city controls matched")
    for _ in range(3):
        x0, sx = fit_line([(lon, x) for lon, _, x, _ in controls])
        y0, sy = fit_line([(m, y) for _, m, _, y in controls])
        filtered = [c for c in controls if math.hypot(c[2] - (x0 + sx * c[0]), c[3] - (y0 + sy * c[1])) <= 8]
        if len(filtered) == len(controls) or len(filtered) < 8:
            break
        controls = filtered
    rms = math.sqrt(sum((x - x0 - sx * lon) ** 2 + (y - y0 - sy * m) ** 2 for lon, m, x, y in controls) / len(controls))
    return x0, sx, y0, sy, len(controls), rms


def province_samples(province: dict, raster) -> list[tuple[int, int]]:
    x, y = province["centroid"]
    width, height = raster.size
    points = []
    offsets = [(0, 0), (-4, 0), (4, 0), (0, -4), (0, 4),
               (-12, 0), (12, 0), (0, -12), (0, 12),
               (-8, -8), (8, -8), (-8, 8), (8, 8)]
    for dx, dy in offsets:
        px, py = x + dx, y + dy
        if not (0 <= px < width and 0 <= py < height):
            continue
        rgb = raster.getpixel((px, py))[:3]
        if (rgb[0] << 16) | (rgb[1] << 8) | rgb[2] == province["id"]:
            points.append((px, py))
    return points


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--game-root", type=Path, default=GAME_ROOT)
    parser.add_argument("--region", default="europe")
    parser.add_argument("--output", type=Path, required=True, help="Local CSV report path")
    args = parser.parse_args()
    try:
        import javaobj
        from PIL import Image
        from shapely.geometry import Point, Polygon
        from shapely.strtree import STRtree
    except ImportError as exc:
        parser.error(f"install javaobj-py3 shapely pillow: {exc}")

    root = Path(__file__).resolve().parents[2]
    map_dir = root / "data/map" / args.region
    geojson = root / "tools/map/inputs" / args.region / "populated_places.geojson"
    x0, sx, y0, sy, controls, rms = calibrate(args.game_root, geojson)
    graph = json.loads((map_dir / "provinces.json").read_text(encoding="utf-8"))
    raster = Image.open(map_dir / "provinces.png").convert("RGB")
    scenario_dir = args.game_root / "map/Earth/scenarios/1440"
    with (scenario_dir / "1440").open("rb") as stream:
        tags = javaobj.load(stream).lCivsTags
    with (scenario_dir / "1440_PD").open("rb") as stream:
        owner_ids = [int(owner.value) for owner in javaobj.load(stream).lProvinceOwners]
    if len(owner_ids) != 4269 or max(owner_ids) > len(tags):
        raise ValueError("unexpected AoC2 province/owner table layout")

    polygons = []
    ids = []
    for province_id in range(len(owner_ids)):
        with (args.game_root / "map/Earth/data/provinces" / str(province_id)).open("rb") as stream:
            source = javaobj.load(stream)
        points = [(float(x.value), float(y.value)) for x, y in zip(source.lPointsX, source.lPointsY)]
        if len(points) < 3:
            continue
        poly = Polygon(points)
        if poly.is_empty:
            continue
        polygons.append(poly)
        ids.append(province_id)
    tree = STRtree(polygons)

    def owner_at(x: float, y: float) -> str | None:
        point = Point(x, y)
        candidates = [int(i) for i in tree.query(point) if polygons[int(i)].covers(point)]
        if not candidates:
            return None
        province_id = ids[min(candidates, key=lambda i: polygons[i].area)]
        owner = owner_ids[province_id]
        return str(tags[owner - 1]) if owner else None

    london = owner_at(2085, 881)
    if london != "engk_m":
        raise ValueError(f"owner table index validation failed: London={london!r}, expected engk_m")

    origin_x, origin_y = graph["projection"]["origin"]
    scale = graph["projection"]["pixels_per_degree"]
    current_owner = {}
    for country_path in (root / "data/countries").glob("*.json"):
        country = json.loads(country_path.read_text(encoding="utf-8"))
        for key in country["start"]["provinces"]:
            current_owner[key] = country["tag"]
    rows = []
    for province in graph["provinces"]:
        if province["kind"] != "land":
            continue
        samples = province_samples(province, raster)
        matches = []
        for px, py in samples:
            lon = (px + origin_x) / scale - 180
            lat = 90 - (py + origin_y) / scale
            owner = owner_at(x0 + sx * lon, y0 + sy * mercator(lat))
            if owner:
                matches.append(owner)
        ranked = Counter(matches).most_common()
        best = ranked[0][0] if ranked else ""
        rows.append({"province_id": province["id"], "province_key": province["key"],
                     "current_owner": current_owner.get(province["key"], ""),
                     "aoc2_tag": best, "confidence": round(ranked[0][1] / len(samples), 3) if ranked and samples else 0,
                     "matched_samples": len(matches), "total_samples": len(samples),
                     "other_tags": ";".join(f"{tag}:{n}" for tag, n in ranked[1:])})
    args.output.parent.mkdir(parents=True, exist_ok=True)
    with args.output.open("w", encoding="utf-8", newline="") as stream:
        writer = csv.DictWriter(stream, fieldnames=list(rows[0]))
        writer.writeheader()
        writer.writerows(rows)
    print(json.dumps({"city_controls": controls, "projection_rms_px": round(rms, 2),
                      "projection": [round(x0, 4), round(sx, 4), round(y0, 4), round(sy, 4)],
                      "london_tag": london, "land_provinces": len(rows),
                      "high_confidence": sum(row["confidence"] >= 0.75 for row in rows),
                      "unmatched": sum(not row["aoc2_tag"] for row in rows),
                      "output": str(args.output)}, ensure_ascii=False))


if __name__ == "__main__":
    main()
