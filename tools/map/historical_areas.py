"""Source provenance and reproducible 1440 starting ownership for the dense slice."""

from collections import Counter, defaultdict
import re
import unicodedata

import numpy as np

from map_common import BOUNDS, HEIGHT, INPUTS, ROOT, WIDTH, json_bytes, read_json

HISTORY_SOURCE = "https://github.com/Seshat-Global-History-Databank/cliopatria/releases/tag/v0.1.3"


def slug(name):
    ascii_name = unicodedata.normalize("NFKD", name).encode("ascii", "ignore").decode()
    return re.sub(r"[^a-z0-9]+", "_", ascii_name.lower()).strip("_")


def geographic_region(kind, pixel):
    y, x = pixel
    west, south, east, north = BOUNDS
    lon = west + (x + 0.5) * (east - west) / WIDTH
    lat = north - (y + 0.5) * (north - south) / HEIGHT
    if kind == "lake":
        return "alpine_europe", "Alpine Europe"
    if kind == "sea":
        if lat < 44 and lon >= -5.6:
            return "western_mediterranean", "Western Mediterranean"
        if lat >= 50 and lon >= 0:
            return "north_sea", "North Sea"
        return "eastern_atlantic", "Eastern Atlantic"
    if lat < 38 and lon >= -1.4:
        return "north_africa", "North Africa"
    if lat >= 49.8 and lon < 2:
        return "british_and_irish_isles", "British and Irish Isles"
    if lat < 43.2 and lon < 3.3:
        return "iberia", "Iberia"
    if 6.5 <= lon < 10.0 and 38 <= lat < 43.15:
        return "western_mediterranean", "Western Mediterranean"
    if lat >= 54.8 and lon >= 8:
        return "scandinavia", "Scandinavia"
    if lat >= 49 and lon >= 3:
        return "northern_continental_europe", "Northern Continental Europe"
    if lat < 49 and lon >= 6:
        return "alpine_europe", "Alpine Europe"
    return "western_continental_europe", "Western Continental Europe"


def overlapping_polity(history, ids, province, historical_features):
    pixels = history[ids == province]
    counts = Counter()
    for sample, amount in enumerate(np.bincount(pixels)):
        if sample and amount:
            name = historical_features[sample - 1]["properties"]["name"]
            counts[name] += int(amount)
    if counts:
        return min(counts, key=lambda name: (-counts[name], name))
    return None


def classify_seeds(seeds, ids, history, historical_features, config):
    source = read_json(INPUTS / "administrative_areas.json")
    land_keys = {key for key, kind, _ in seeds.values() if kind == "land"}
    regions, areas, assignments, ownership = {}, {}, {}, {}
    by_city = {}
    for area in source["areas"]:
        for city in area["cities"]:
            if city not in land_keys or city in by_city:
                raise ValueError(f"unknown or duplicate administrative city {city}")
            by_city[city] = area
    for city in config["ownership_overrides"]:
        if city not in land_keys:
            raise ValueError(f"unknown ownership override city {city}")

    for province, (key, kind, pixel) in seeds.items():
        region, region_name = geographic_region(kind, pixel)
        if kind != "land":
            area_id = f"{region}_{kind}"
            name = f"{region_name} Waters" if kind == "sea" else "Lake Geneva"
            area_kind, citation = "water", None
        else:
            polity = overlapping_polity(history, ids, province, historical_features)
            if key in source["ignored_polity_matches"]:
                polity = None
            tag = config["country_polities"].get(polity)
            override = config["ownership_overrides"].get(key)
            if override is not None:
                tag = override["owner"]
            ownership[key] = tag

            area = by_city.get(key)
            if area is not None:
                area_id, name, citation = area["key"], area["name"], source["sources"][area["source"]]
                region, region_name = area["region"], area["region_name"]
                area_kind = area["kind"]
            elif polity in source["historical_polities"]:
                # Preserve a source-backed local jurisdiction where detailed
                # administrative subdivisions are unavailable.
                area_id, name, area_kind, citation = f"{region}_{slug(polity)}", polity, "historical_polity", HISTORY_SOURCE
            else:
                area_id = f"{region}_unclassified"
                name, area_kind, citation = f"{region_name} (Unclassified)", "unclassified", None

        regions.setdefault(region, {"key": region, "name_key": f"region_{region}", "name": region_name})
        areas.setdefault(area_id, {
            "key": area_id, "name_key": f"area_{area_id}", "name": name, "region": region,
            "kind": area_kind, "source": citation,
        })
        assignments[key] = area_id

    metadata = {
        "schema_version": 1,
        "reference_year": 1440,
        "assignment_method": (
            "Present-day settlement seeds form gameplay cells. Named historical areas "
            "group selected settlements using cited records and approximate membership. "
            "The map does not reproduce historical boundary lines or modern administrative borders."
        ),
        "areas": sorted(areas.values(), key=lambda entry: entry["key"]),
    }
    region_metadata = {"schema_version": 1, "regions": sorted(regions.values(), key=lambda entry: entry["key"])}
    return assignments, metadata, region_metadata, ownership


def update_ownership(ownership):
    capitals = {
        "ara": "zaragoza", "cas": "valladolid", "eng": "london",
        "fra": "paris", "por": "lisbon", "sco": "edinburgh",
    }
    grouped = defaultdict(list)
    for key, tag in ownership.items():
        grouped[tag].append(key)

    countries = []
    for tag, capital in capitals.items():
        path = ROOT / "data" / "countries" / f"{tag}.json"
        record = read_json(path)
        if ownership.get(capital) != tag:
            raise ValueError(f"{tag}: capital {capital} is outside starting ownership")
        record["capital"] = capital
        record["start"]["provinces"] = sorted(grouped[tag])
        countries.append((path, record))

    path = ROOT / "data" / "scenarios" / "1440.json"
    scenario = read_json(path)
    scenario["unowned_provinces"] = sorted(grouped[None])
    forts = scenario["military"]["forts"]
    relocated = {
        "aragon": "zaragoza", "castile": "valladolid", "ile_de_france": "paris",
        "lisboa": "lisbon", "lowlands": "edinburgh", "normandie": "rouen",
    }
    scenario["military"]["forts"] = {
        relocated.get(key, key): value for key, value in forts.items()
    }
    for entry_path, record in (*countries, (path, scenario)):
        entry_path.write_bytes(json_bytes(record, pretty=True))
