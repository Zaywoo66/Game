#!/usr/bin/env python3
"""Seed the 1440 scenario from majority matches in the historical map sidecar."""

import argparse
from collections import Counter
from copy import deepcopy
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
MAP_PATH = ROOT / "data/map/europe/provinces.json"
EVIDENCE_PATH = ROOT / "data/map/europe/polities_1440.json"
SCENARIO_PATH = ROOT / "data/scenarios/1440.json"
COUNTRY_DIR = ROOT / "data/countries"
OWNERSHIP_POLICY_PATH = ROOT / "data/map/europe/ownership_1440.json"
ALIASES = {"Burgundian Netherlands": "bur", "Hafsid Dynasty": "haf", "House of Habsburg": "aus"}


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, value):
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def majority_owner_updates(scenario, countries, assignments, threshold=0.5):
    """Return copied scenario/country data with eligible, unowned cells assigned."""
    if not 0.3 <= threshold <= 1.0:
        raise ValueError("threshold must be between 0.3 and 1.0")
    updated_scenario = deepcopy(scenario)
    updated_countries = deepcopy(countries)
    polity_to_tag = {country["name"]: tag for tag, country in updated_countries.items()}
    polity_to_tag.update({name: tag for name, tag in ALIASES.items() if tag in updated_countries})
    unowned = set(updated_scenario["unowned_provinces"])
    already_owned = {
        province
        for country in updated_countries.values()
        for province in country["start"]["provinces"]
    }
    assigned = Counter()

    for row in assignments:
        province = row["province_key"]
        tag = polity_to_tag.get(row["polity_name"])
        pixels = row["province_pixels"]
        if (tag is None or row["resolution"] != "polygon" or pixels <= 0
                or row["overlap_pixels"] / pixels < threshold or province not in unowned
                or province in already_owned):
            continue
        updated_countries[tag]["start"]["provinces"].append(province)
        unowned.remove(province)
        assigned[tag] += 1

    for tag in assigned:
        updated_countries[tag]["start"]["provinces"] = sorted(
            set(updated_countries[tag]["start"]["provinces"])
        )
    # Preserve the content file's ordering for unchanged countries and neutral
    # provinces; a dry run must not create unrelated data churn.
    updated_scenario["unowned_provinces"] = [
        key for key in updated_scenario["unowned_provinces"] if key in unowned
    ]
    return updated_scenario, updated_countries, assigned



def kalmar_owner_for_province(key, province, rules):
    """Split coarse Kalmar evidence into separate owner-requested countries."""
    if key in rules["denmark_override_keys"]:
        return rules["kalmar_country_tags"]["denmark"]
    if key in rules["norway_override_keys"]:
        return rules["kalmar_country_tags"]["norway"]
    x, y = province["centroid"]
    longitude = -13.0 + (x + 0.5) / 32.0
    latitude = 72.0 - (y + 0.5) / 32.0
    points = rules["norway_sweden_border_lat_lon"]
    if latitude < points[0][0] or latitude > points[-1][0]:
        return rules["kalmar_country_tags"]["sweden"]
    for lower, upper in zip(points, points[1:]):
        if lower[0] <= latitude <= upper[0]:
            ratio = (latitude - lower[0]) / (upper[0] - lower[0])
            border_lon = lower[1] + ratio * (upper[1] - lower[1])
            return (rules["kalmar_country_tags"]["norway"] if longitude < border_lon
                    else rules["kalmar_country_tags"]["sweden"])
    return rules["kalmar_country_tags"]["sweden"]

def owner_directed_assignments(map_data, evidence, policy):
    """Resolve the owner's explicit steppe partition into province-key tags."""
    rules = policy["rules"]
    province_by_key = {row["key"]: row for row in map_data["provinces"]}
    assignments = {row["province_key"]: row for row in evidence["assignments"]}
    north_caucasus = set(rules["north_caucasus_keys"])
    if not north_caucasus <= province_by_key.keys():
        raise ValueError("north Caucasus override references an unknown province")
    kalmar_overrides = set(rules.get("denmark_override_keys", [])) | set(
        rules.get("norway_override_keys", [])
    )
    if not kalmar_overrides <= province_by_key.keys():
        raise ValueError("Scandinavian ownership override references an unknown province")
    if any(rules["kalmar_country_tags"][name] not in {"den", "nor", "swe"}
           for name in ("denmark", "norway", "sweden")):
        raise ValueError("Kalmar partition must map to the independent Denmark/Norway/Sweden tags")
    resolved = {}
    for key, row in assignments.items():
        polity = row["polity_name"]
        if polity == rules["golden_horde_source_name"]:
            if key in north_caucasus:
                tag = rules["north_caucasus_tag"]
            else:
                x = province_by_key[key]["centroid"][0]
                longitude = -13.0 + (x + 0.5) / 32.0
                if longitude < rules["middle_west_longitude_degrees"]:
                    tag = rules["western_tag"]
                elif longitude < rules["eastern_longitude_degrees"]:
                    tag = rules["middle_tag"]
                else:
                    tag = rules["eastern_tag"]
            resolved[key] = tag
        elif polity in rules["source_polity_tags"]:
            resolved[key] = rules["source_polity_tags"][polity]
        elif polity == rules.get("kalmar_source_name") and row["resolution"] == "coarse_group":
            resolved[key] = kalmar_owner_for_province(key, province_by_key[key], rules)
    for key, tag in rules.get("capital_proxy_tags", {}).items():
        if key not in province_by_key or province_by_key[key]["kind"] != "land":
            raise ValueError(f"capital proxy is not an existing land province: {key}")
        resolved[key] = tag
    for key, tag in rules["explicit_province_tags"].items():
        if key not in province_by_key or province_by_key[key]["kind"] != "land":
            raise ValueError(f"ownership override is not an existing land province: {key}")
        resolved[key] = tag
    return resolved


def apply_owner_directed_assignments(scenario, countries, resolved):
    """Apply explicit user-approved mappings, moving keys out of any prior owner."""
    updated_scenario = deepcopy(scenario)
    updated_countries = deepcopy(countries)
    unowned = set(updated_scenario["unowned_provinces"])
    ownership = {
        province: tag
        for tag, country in updated_countries.items()
        for province in country["start"]["provinces"]
    }
    assigned = Counter()
    for province, tag in sorted(resolved.items()):
        if tag not in updated_countries:
            raise ValueError(f"ownership policy requires missing country content: {tag}")
        previous = ownership.get(province)
        if previous and previous != tag:
            updated_countries[previous]["start"]["provinces"].remove(province)
        if province not in updated_countries[tag]["start"]["provinces"]:
            updated_countries[tag]["start"]["provinces"].append(province)
        ownership[province] = tag
        unowned.discard(province)
        assigned[tag] += 1
    for tag, country in updated_countries.items():
        country["start"]["provinces"] = sorted(set(country["start"]["provinces"]))
        if country["capital"] not in country["start"]["provinces"]:
            raise ValueError(f"{tag} capital is not among its owned provinces")
    updated_scenario["unowned_provinces"] = [
        key for key in updated_scenario["unowned_provinces"] if key in unowned
    ]
    return updated_scenario, updated_countries, assigned


def nearest_owner_updates(scenario, countries, map_data, evidence, rules):
    """Fill every remaining land cell by named overlap or nearest owned graph cell.

    HRE coarse cells are restricted to neighboring represented HRE members so
    they never collapse into a synthetic single Empire country.
    """
    updated_scenario = deepcopy(scenario)
    updated_countries = deepcopy(countries)
    provinces = {row["key"]: row for row in map_data["provinces"]}
    by_id = {row["id"]: row for row in map_data["provinces"]}
    existing_owner = {
        key: tag
        for tag, country in updated_countries.items()
        for key in country["start"]["provinces"]
    }
    hre_member_tags = set(rules["fallback"]["hre_owner_tags"])
    hre_keys = {
        row["province_key"]
        for row in evidence["assignments"]
        if row.get("resolution") == "coarse_group"
        and row.get("polity_name") == "Holy Roman Empire Minor States"
        and row["province_key"] not in rules.get("explicit_province_tags", {})
        and existing_owner.get(row["province_key"]) not in hre_member_tags
    }
    for country in updated_countries.values():
        country["start"]["provinces"] = [
            key for key in country["start"]["provinces"] if key not in hre_keys
        ]
    owner = {
        key: tag
        for tag, country in updated_countries.items()
        for key in country["start"]["provinces"]
        if key not in hre_keys
    }
    if len(owner) != sum(len(country["start"]["provinces"]) for country in updated_countries.values()):
        raise ValueError("a province has multiple owners before fallback assignment")

    polity_to_tag = {country["name"]: tag for tag, country in updated_countries.items()}
    polity_to_tag.update({name: tag for name, tag in ALIASES.items() if tag in updated_countries})
    evidence_by_key = {row["province_key"]: row for row in evidence["assignments"]}
    named = Counter()
    for key in updated_scenario["unowned_provinces"]:
        row = evidence_by_key.get(key, {})
        tag = polity_to_tag.get(row.get("polity_name"))
        if (tag and row.get("resolution") == "polygon"
                and row.get("overlap_pixels", 0) > 0):
            owner[key] = tag
            named[tag] += 1

    def nearest_tags(eligible_tags):
        allowed = set(eligible_tags)
        if not allowed <= updated_countries.keys():
            missing = sorted(allowed - updated_countries.keys())
            raise ValueError(f"nearest-owner policy references missing countries: {missing}")
        distances = {}
        frontier = []
        for province_key, tag in owner.items():
            if tag in allowed:
                province_id = provinces[province_key]["id"]
                state = (0, tag, province_id)
                if province_id not in distances or state < distances[province_id]:
                    distances[province_id] = state
                    frontier.append(state)
        frontier.sort()
        cursor = 0
        while cursor < len(frontier):
            distance, tag, province_id = frontier[cursor]
            cursor += 1
            if distances.get(province_id) != (distance, tag, province_id):
                continue
            for edge in by_id[province_id]["neighbors"]:
                target = edge["id"]
                candidate = (distance + 1, tag, target)
                current = distances.get(target)
                if current is None or candidate[:2] < current[:2]:
                    distances[target] = candidate
                    frontier.append(candidate)
        return {province_id: state[1] for province_id, state in distances.items()}

    all_tags = set(updated_countries)
    all_nearest = nearest_tags(all_tags)
    hre_tags = set(rules["fallback"]["hre_owner_tags"])
    hre_nearest = nearest_tags(hre_tags)
    assigned_nearest = Counter()
    unresolved = (set(updated_scenario["unowned_provinces"]) - set(owner)) | hre_keys
    for key in sorted(unresolved):
        row = evidence_by_key.get(key, {})
        province_id = provinces[key]["id"]
        if row.get("resolution") == "coarse_group" and row.get("polity_name") == "Holy Roman Empire Minor States":
            tag = hre_nearest.get(province_id)
            if tag is None:
                raise ValueError(f"no represented HRE owner can reach province {key}")
            assigned_nearest["hre:" + tag] += 1
        else:
            tag = all_nearest.get(province_id)
            if tag is None:
                raise ValueError(f"no owner can reach province {key}")
            assigned_nearest[tag] += 1
        owner[key] = tag

    for key, tag in owner.items():
        if key not in updated_countries[tag]["start"]["provinces"]:
            updated_countries[tag]["start"]["provinces"].append(key)
    for country in updated_countries.values():
        country["start"]["provinces"] = sorted(set(country["start"]["provinces"]))
        if country["capital"] not in country["start"]["provinces"]:
            raise ValueError(f"{country['tag']} capital is not among its owned provinces")
    updated_scenario["unowned_provinces"] = []
    return updated_scenario, updated_countries, named, assigned_nearest


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--threshold", type=float, default=0.5,
                        help="minimum source-polygon share; default assigns majority matches")
    parser.add_argument("--apply", action="store_true", help="write scenario and country ownership")
    parser.add_argument("--fill-unowned", action="store_true",
                        help="assign remaining cells by nearest-owner fallback")
    args = parser.parse_args()
    map_data = read_json(MAP_PATH)
    evidence = read_json(EVIDENCE_PATH)
    policy = read_json(OWNERSHIP_POLICY_PATH)
    scenario = read_json(SCENARIO_PATH)
    map_identity = (map_data["map_version"], map_data["map_hash"])
    if map_identity != (evidence["map_version"], evidence["map_hash"]):
        raise SystemExit("1440 evidence does not match the current Europe map identity")
    if map_identity != (policy["map_version"], policy["map_hash"]):
        raise SystemExit("1440 ownership policy does not match the current Europe map identity")

    countries = {tag: read_json(COUNTRY_DIR / f"{tag}.json") for tag in scenario["countries"]}
    try:
        scenario, countries, assigned = majority_owner_updates(
            scenario, countries, evidence["assignments"], args.threshold
        )
        directed = owner_directed_assignments(map_data, evidence, policy)
        scenario, countries, directed_counts = apply_owner_directed_assignments(
            scenario, countries, directed
        )
        named_counts = Counter()
        fallback_counts = Counter()
        if args.fill_unowned:
            scenario, countries, named_counts, fallback_counts = nearest_owner_updates(
                scenario, countries, map_data, evidence, policy["rules"]
            )
    except ValueError as error:
        parser.error(str(error))
    remaining = len(scenario["unowned_provinces"])
    print(f"majority threshold={args.threshold:.0%}; source assignments={sum(assigned.values())}; owner-directed assignments={sum(directed_counts.values())}; remaining neutral={remaining}")
    for tag, amount in sorted(assigned.items()):
        print(f"{tag}: +{amount}")
    if args.fill_unowned:
        print(f"low-confidence named matches={sum(named_counts.values())}; nearest-owner fallback={sum(fallback_counts.values())}")
        for tag, amount in sorted(fallback_counts.items()):
            print(f"{tag}: +{amount}")

    if args.apply:
        for tag, country in countries.items():
            write_json(COUNTRY_DIR / f"{tag}.json", country)
        write_json(SCENARIO_PATH, scenario)
    else:
        print("dry run; pass --apply to write the scenario")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
