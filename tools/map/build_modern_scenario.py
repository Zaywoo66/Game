"""Build a 2022 sovereign-state scenario from Natural Earth boundaries."""

from __future__ import annotations

import argparse
from collections import defaultdict
from hashlib import sha256
from io import BytesIO
import json
from pathlib import Path
import re
import unicodedata
from zipfile import ZipFile

import numpy as np
import shapefile
from PIL import Image
from rasterio.features import rasterize
from rasterio.transform import from_bounds
from scipy import ndimage
from shapely.geometry import box, shape
from shapely.ops import unary_union

from color_1440_countries import assign_colors
from map_common import ROOT, json_bytes, read_json


MAP_JSON = ROOT / "data/map/europe/provinces.json"
MAP_IMAGE = ROOT / "data/map/europe/provinces.png"
SCENARIO = ROOT / "data/scenarios/2022.json"
COUNTRIES = ROOT / "data/countries"
CHARACTERS = ROOT / "data/characters"
OUTPUT = ROOT / "data/map/europe/ownership_2022.json"
BORDER_RASTER = ROOT / "data/map/europe/country_borders_2022.png"
COUNTRY_FILL_RASTER = ROOT / "data/map/europe/country_fill_2022.png"
INPUTS = ROOT / "tools/map/inputs/modern"
COUNTRY_ZIP = INPUTS / "ne_10m_admin_0_countries.zip"
UKRAINE_POV_ZIP = INPUTS / "ne_10m_admin_0_countries_ukraine_pov.zip"
PLACES_ZIP = INPUTS / "ne_10m_populated_places.zip"
COUNTRY_STEM = "ne_10m_admin_0_countries_ukr"
PLACES_STEM = "ne_10m_populated_places"
BOUNDS = (-18.0, 10.0, 68.0, 72.0)
WIDTH, HEIGHT = 2752, 1984
SHAPE = (HEIGHT, WIDTH)
HUMAN_TAG = "modern_gb1"
RULER_KEY = "queen_elizabeth_ii_2022"
HEIR_KEY = "charles_iii_2022"

GOVERNMENTS = {
    "GB1": "monarchy",
    "DN1": "monarchy",
    "NL1": "monarchy",
    "BEL": "monarchy",
    "ESP": "monarchy",
    "NOR": "monarchy",
    "SWE": "monarchy",
    "LUX": "monarchy",
    "MCO": "monarchy",
    "LIE": "monarchy",
    "AND": "monarchy",
    "VAT": "theocracy",
    "MAR": "monarchy", "SAU": "monarchy", "OMN": "monarchy", "ARE": "monarchy",
    "QAT": "monarchy", "BHR": "monarchy", "KWT": "monarchy", "JOR": "monarchy",
}
LEADERS = {
    # State or government heads on 2022-01-01. Birth years are intentionally
    # year-precision where sources disagree or do not establish a day.
    "MAR": ("Mohammed VI", "mohammed_vi", 1963, "male", "alaouite"),
    "SAH": ("Brahim Ghali", "brahim_ghali", 1949, "male", "polisario"),
    "DZA": ("Abdelmadjid Tebboune", "abdelmadjid_tebboune", 1945, "male", "tebboune"),
    "TUN": ("Kais Saied", "kais_saied", 1958, "male", "saied"),
    "MRT": ("Mohamed Ould Ghazouani", "mohamed_ould_ghazouani", 1956, "male", "ghazouani"),
    "MLI": ("Assimi Goïta", "assimi_goita", 1983, "male", "goita"),
    "NER": ("Mohamed Bazoum", "mohamed_bazoum", 1960, "male", "bazoum"),
    "TCD": ("Mahamat Idriss Déby Itno", "mahamat_deby", 1984, "male", "deby"),
    "LBY": ("Mohamed al-Menfi", "mohamed_al_menfi", 1976, "male", "menfi"),
    "EGY": ("Abdel Fattah el-Sisi", "abdel_fattah_el_sisi", 1954, "male", "sisi"),
    "SDN": ("Abdel Fattah al-Burhan", "abdel_fattah_al_burhan", 1960, "male", "burhan"),
    "SAU": ("Salman bin Abdulaziz Al Saud", "salman_bin_abdulaziz", 1935, "male", "al_saud"),
    "YEM": ("Abdrabbuh Mansur Hadi", "abdrabbuh_mansur_hadi", 1945, "male", "hadi"),
    "OMN": ("Haitham bin Tariq", "haitham_bin_tariq", 1954, "male", "al_said"),
    "ARE": ("Khalifa bin Zayed Al Nahyan", "khalifa_bin_zayed", 1948, "male", "al_nahyan"),
    "QAT": ("Tamim bin Hamad Al Thani", "tamim_bin_hamad", 1980, "male", "al_thani"),
    "BHR": ("Hamad bin Isa Al Khalifa", "hamad_bin_isa", 1950, "male", "al_khalifa"),
    "KWT": ("Nawaf Al-Ahmad Al-Jaber Al-Sabah", "nawaf_al_sabah", 1937, "male", "al_sabah"),
    "JOR": ("Abdullah II", "abdullah_ii", 1962, "male", "hashemite"),
    "LBN": ("Michel Aoun", "michel_aoun", 1933, "male", "aoun"),
    "SYR": ("Bashar al-Assad", "bashar_al_assad", 1965, "male", "assad"),
    "IRQ": ("Barham Salih", "barham_salih", 1960, "male", "salih"),
    "IRN": ("Ali Khamenei", "ali_khamenei", 1939, "male", "khamenei"),
    "IS1": ("Isaac Herzog", "isaac_herzog", 1960, "male", "herzog"),
}
NAME_OVERRIDES = {
    "Republic of Serbia": "Serbia",
    "Republic of Moldova": "Moldova",
    "Russian Federation": "Russia",
    "Turkey": "Turkey",
    "Vatican": "Vatican City",
    "United States of America": "United States",
}


def reader_from_zip(path: Path, stem: str) -> shapefile.Reader:
    with ZipFile(path) as archive:
        return shapefile.Reader(
            shp=BytesIO(archive.read(f"{stem}.shp")),
            shx=BytesIO(archive.read(f"{stem}.shx")),
            dbf=BytesIO(archive.read(f"{stem}.dbf")),
            encoding="utf-8",
        )


def decode_ids(path: Path) -> np.ndarray:
    rgb = np.asarray(Image.open(path).convert("RGB"), dtype=np.uint32)
    return (rgb[:, :, 0] << 16) | (rgb[:, :, 1] << 8) | rgb[:, :, 2]


def read_groups() -> tuple[list[dict], dict[str, tuple[float, float]], list[dict]]:
    clip = box(*BOUNDS)
    reader = reader_from_zip(UKRAINE_POV_ZIP, COUNTRY_STEM)
    fields = [field[0] for field in reader.fields[1:]]
    groups: dict[str, dict] = {}
    code_by_name: dict[str, str] = {}
    for record, item in zip(reader.records(), reader.shapes()):
        attrs = dict(zip(fields, record))
        code = str(attrs["SOV_A3"]).strip()
        sovereign = str(attrs["SOVEREIGNT"]).strip()
        if not code or code == "-99" or not sovereign:
            continue
        geometry = shape(item.__geo_interface__).intersection(clip)
        if geometry.is_empty:
            continue
        group = groups.setdefault(code, {"code": code, "name": sovereign, "geometries": []})
        if group["name"] != sovereign:
            raise ValueError(f"Natural Earth code {code} maps to multiple sovereigns")
        group["geometries"].append(geometry)
        code_by_name[sovereign] = code

    places_reader = reader_from_zip(PLACES_ZIP, "ne_10m_populated_places")
    place_fields = [field[0] for field in places_reader.fields[1:]]
    capitals: dict[str, tuple[float, float]] = {}
    places: list[dict] = []
    for record, item in zip(places_reader.records(), places_reader.shapes()):
        attrs = dict(zip(place_fields, record))
        code = str(attrs["SOV_A3"]).strip()
        if code not in groups:
            code = code_by_name.get(str(attrs["SOV0NAME"]).strip(), code)
        x, y = item.points[0]
        if BOUNDS[0] <= x <= BOUNDS[2] and BOUNDS[1] <= y <= BOUNDS[3]:
            places.append({
                "name": str(attrs.get("NAME", "")).strip(),
                "x": x,
                "y": y,
                "population": max(0, int(float(attrs.get("POP_MAX") or 0))),
                "is_capital": int(attrs.get("ADM0CAP") or 0) == 1,
            })
        if (
            code in groups
            and int(attrs["ADM0CAP"] or 0) == 1
            and BOUNDS[0] <= item.points[0][0] <= BOUNDS[2]
            and BOUNDS[1] <= item.points[0][1] <= BOUNDS[3]
        ):
            capitals.setdefault(code, (x, y))

    result = []
    for code, group in sorted(groups.items()):
        result.append({**group, "geometry": unary_union(group["geometries"])})
    return result, capitals, places


def normalized_name(value: str) -> str:
    folded = unicodedata.normalize("NFKD", value).encode("ascii", "ignore").decode("ascii")
    return re.sub(r"[^a-z0-9]", "", folded.lower())


def build() -> tuple[dict, dict[str, dict], dict, np.ndarray, np.ndarray]:
    map_data = read_json(MAP_JSON)
    ids = decode_ids(MAP_IMAGE)
    if ids.shape != SHAPE:
        raise ValueError(f"unexpected province raster dimensions: {ids.shape}")
    provinces = map_data["provinces"]
    land_ids = {row["id"] for row in provinces if row["kind"] == "land"}
    land_mask = np.isin(ids, np.fromiter(land_ids, dtype=np.uint32))
    groups, capital_points, places = read_groups()

    shapes = [(row["geometry"], index) for index, row in enumerate(groups, start=1)]
    owner_raster = rasterize(
        shapes,
        out_shape=SHAPE,
        transform=from_bounds(*BOUNDS, WIDTH, HEIGHT),
        fill=0,
        dtype=np.uint16,
        all_touched=False,
    )
    missing = land_mask & (owner_raster == 0)
    if np.any(missing):
        nearest = ndimage.distance_transform_edt(owner_raster == 0, return_distances=False, return_indices=True)
        owner_raster[missing] = owner_raster[nearest[0][missing], nearest[1][missing]]

    width = len(groups) + 1
    counts = np.bincount(
        (ids[land_mask].astype(np.int64) * width + owner_raster[land_mask]),
        minlength=(max(land_ids) + 1) * width,
    ).reshape(max(land_ids) + 1, width)
    selected = counts.argmax(axis=1)
    province_owner: dict[int, int] = {}
    for province_id in sorted(land_ids):
        owner_id = int(selected[province_id])
        if owner_id == 0:
            raise ValueError(f"no sovereign owner rasterized for land province {province_id}")
        province_owner[province_id] = owner_id - 1

    # Named settlement cells follow the sovereign polygon containing the
    # settlement, rather than whichever neighbor covers a slightly larger
    # share of the surrounding cell.
    province_by_name: dict[str, int] = {}
    for province in provinces:
        key = province["key"]
        if province["kind"] != "land" or re.search(r"_part_\d+$", key) or key.startswith("eu_"):
            continue
        province_by_name[normalized_name(key)] = province["id"]
    anchor_candidates: dict[int, list[tuple[int, int, int, str]]] = defaultdict(list)
    for place in places:
        province_id = province_by_name.get(normalized_name(place["name"]))
        if province_id is None:
            continue
        x = min(WIDTH - 1, max(0, int((place["x"] - BOUNDS[0]) / (BOUNDS[2] - BOUNDS[0]) * WIDTH)))
        y = min(HEIGHT - 1, max(0, int((BOUNDS[3] - place["y"]) / (BOUNDS[3] - BOUNDS[1]) * HEIGHT)))
        owner_index = int(owner_raster[y, x]) - 1
        if owner_index >= 0:
            anchor_candidates[province_id].append((
                int(place["is_capital"]), place["population"], owner_index, place["name"]
            ))
    anchor_assignments: dict[int, tuple[int, str]] = {}
    for province_id, candidates in anchor_candidates.items():
        _, _, owner_index, name = max(candidates, key=lambda item: (item[0], item[1], -item[2], item[3]))
        province_owner[province_id] = owner_index
        anchor_assignments[province_id] = (owner_index, name)

    country_rows: dict[str, dict] = {}
    for index, group in enumerate(groups):
        leader = LEADERS.get(group["code"])
        country_rows[group["code"]] = {
            "schema_version": 1,
            "tag": f"modern_{group['code'].lower()}",
            "name": NAME_OVERRIDES.get(group["name"], group["name"]),
            "color": [128, 128, 128],
            "capital": None,
            "ai_profile": "balanced",
            "government": GOVERNMENTS.get(group["code"], "republic"),
            "start": {
                "ruler": RULER_KEY if group["code"] == "GB1" else (
                    f"leader_2022_{group['code'].lower()}" if leader else None
                ),
                "heir": HEIR_KEY if group["code"] == "GB1" else None,
                "provinces": [],
                "treasury": 120000,
                "tax_rate": 250,
                "monthly_spending": 12500,
            },
        }

    by_id = {row["id"]: row for row in provinces}
    by_key = {row["key"]: row for row in provinces}
    for province_id, owner_index in province_owner.items():
        group = groups[owner_index]
        country = country_rows[group["code"]]
        country["start"]["provinces"].append(by_id[province_id]["key"])

    kept_codes = {code for code, country in country_rows.items() if country["start"]["provinces"]}
    country_rows = {code: country_rows[code] for code in sorted(kept_codes)}

    for code, country in country_rows.items():
        country["start"]["provinces"].sort()
        own = [by_key[key] for key in country["start"]["provinces"]]
        lonlat = capital_points.get(code)
        if lonlat is not None:
            lon, lat = lonlat
            capital_id = min(
                (row["id"] for row in own),
                key=lambda pid: ((by_id[pid]["centroid"][0] - (lon - BOUNDS[0]) * 32) ** 2
                                 + (by_id[pid]["centroid"][1] - (BOUNDS[3] - lat) * 32) ** 2, pid),
            )
        else:
            capital_id = min(own, key=lambda row: (-row["development"], row["id"]))["id"]
        country["capital"] = by_id[capital_id]["key"]

    scenario = {
        "schema_version": 1,
        "start_date": [2022, 1, 1],
        "seed": 2022,
        "countries": sorted(country["tag"] for country in country_rows.values()),
        "characters": [RULER_KEY, HEIR_KEY] + sorted(
            f"leader_2022_{code.lower()}" for code in country_rows if code in LEADERS
        ),
        "human_slots": {HUMAN_TAG: 1},
        "rules": {
            "adulthood_years": 18,
            "base_taxable_development_per_province": 1,
            "tax_per_development": 20000,
            "max_tax_rate": 500,
            "max_monthly_spending": 20000,
            "debt_limit": 100000,
            "income_modifier": "tax_income",
        },
        "traits": {"administrator": [{"key": "tax_income", "value": 100, "kind": "multiplicative"}],
                   "mercantile_court": [{"key": "tax_income", "value": 500, "kind": "additive"}]},
        "military": {"rules": {
            "raise_cost_per_soldier": 8, "max_army_size": 2000, "max_morale": 1000,
            "morale_recovery_per_tick": 20, "retreat_morale_threshold": 250,
            "winner_casualty_rate": 90, "loser_casualty_rate": 220, "battle_morale_loss": 340,
            "base_supply": 1200, "supply_per_development": 300, "attrition_rate": 25,
            "siege_days_per_level": 15, "battle_score": 20, "occupation_score": 45,
            "peace_score": 40, "truce_ticks": 365, "combat_modifier": "army_combat",
            "morale_modifier": "army_morale",
        }, "forts": {}},
        "navy": {"fleets": []},
        "unowned_provinces": [],
    }

    countries = {country["tag"]: country for country in country_rows.values()}
    countries, _ = assign_colors(scenario, countries, map_data)
    country_fill = np.zeros((*SHAPE, 4), dtype=np.uint8)
    for index, group in enumerate(groups, start=1):
        country = countries.get(f"modern_{group['code'].lower()}")
        if country is None:
            continue
        mask = owner_raster == index
        country_fill[mask, :3] = country["color"]
        country_fill[mask, 3] = 255
    assignment_by_id = {
        str(province_id): country_rows[groups[province_owner[province_id]]["code"]]["tag"]
        for province_id in sorted(province_owner)
    }
    boundary = np.zeros(SHAPE, dtype=bool)
    source_land = owner_raster > 0
    horizontal = source_land[:, 1:] != source_land[:, :-1]
    boundary[:, 1:] |= horizontal
    boundary[:, :-1] |= horizontal
    vertical = source_land[1:, :] != source_land[:-1, :]
    boundary[1:, :] |= vertical
    boundary[:-1, :] |= vertical
    boundary = ndimage.binary_dilation(boundary, structure=np.ones((3, 3), dtype=bool))
    boundary_bytes = boundary.astype(np.uint8) * 255

    metadata = {
        "schema_version": 1,
        "map_version": map_data["map_version"],
        "map_hash": map_data["map_hash"],
        "reference_date": "2022-01-01",
        "source": {
            "name": "Natural Earth Admin-0 Countries (Ukraine POV)",
            "release": "5.1.1",
            "url": "https://www.naturalearthdata.com/downloads/10m-cultural-vectors/",
            "license": "Public domain",
            "file": "tools/map/inputs/modern/ne_10m_admin_0_countries_ukraine_pov.zip",
            "sha256": sha256(UKRAINE_POV_ZIP.read_bytes()).hexdigest(),
        },
        "method": "Rasterize Natural Earth Ukraine POV Admin-0 polygons and group by SOVEREIGNT. Assign each land province by majority raster overlap (ties by source code), then anchor exact matched settlement cells to the polygon containing that settlement point. A softened boundary raster is derived from the source polygons.",
        "province_owners": assignment_by_id,
        "settlement_anchors": {
            str(province_id): {
                "name": name,
                "country": assignment_by_id[str(province_id)],
            }
            for province_id, (_, name) in sorted(anchor_assignments.items())
        },
        "country_tags": {tag: country["name"] for tag, country in sorted(countries.items())},
    }
    return scenario, countries, metadata, boundary_bytes, country_fill


def write_outputs() -> None:
    scenario, countries, metadata, boundary_bytes, country_fill = build()
    scenario_path = SCENARIO
    scenario_path.write_bytes(json_bytes(scenario))
    for country in countries.values():
        (COUNTRIES / f"{country['tag']}.json").write_bytes(json_bytes(country))
    (CHARACTERS / f"{RULER_KEY}.json").write_bytes(json_bytes({
        "schema_version": 2,
        "key": RULER_KEY,
        "country": HUMAN_TAG,
        "name": "Elizabeth II",
        "dynasty": "windsor",
        "sex": "female",
        "birth": [1926, 4, 21],
        "death": [2022, 9, 8],
        "stats": {"adm": 2, "dip": 3, "mil": 2},
        "traits": [],
    }))
    (CHARACTERS / f"{HEIR_KEY}.json").write_bytes(json_bytes({
        "schema_version": 2,
        "key": HEIR_KEY,
        "country": HUMAN_TAG,
        "name": "Charles III",
        "dynasty": "windsor",
        "sex": "male",
        "birth": [1948, 11, 14],
        "stats": {"adm": 3, "dip": 3, "mil": 2},
        "traits": [],
    }))
    for code, (name, key, birth_year, sex, dynasty) in LEADERS.items():
        tag = f"modern_{code.lower()}"
        if tag not in scenario["countries"]:
            continue
        character_key = f"leader_2022_{code.lower()}"
        (CHARACTERS / f"{character_key}.json").write_bytes(json_bytes({
            "schema_version": 2,
            "key": character_key,
            "country": tag,
            "name": name,
            "dynasty": dynasty,
            "sex": sex,
            "birth": [birth_year, 1, 1],
            "birth_precision": "year",
            "stats": {"adm": 2, "dip": 2, "mil": 2},
            "traits": [],
        }))
    OUTPUT.write_bytes(json_bytes(metadata))
    Image.fromarray(boundary_bytes, mode="L").save(BORDER_RASTER, optimize=False)
    Image.fromarray(country_fill, mode="RGBA").save(COUNTRY_FILL_RASTER, optimize=True)
    print(f"wrote {len(countries)} modern country records and {len(metadata['province_owners'])} land owners")
    print(f"map v{metadata['map_version']} {metadata['map_hash']}")


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--apply", action="store_true", help="write the modern 2022 scenario")
    args = parser.parse_args()
    scenario, countries, metadata, _, _ = build()
    print(f"scenario: {len(countries)} sovereign states, {len(metadata['province_owners'])} owned land provinces")
    print("countries:")
    for tag in sorted(countries):
        country = countries[tag]
        print(f"  {tag}: {country['name']} ({len(country['start']['provinces'])} provinces)")
    if args.apply:
        write_outputs()
    else:
        print("dry run; pass --apply to write scenario files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
