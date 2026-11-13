#!/usr/bin/env python3
"""Add reviewed 1440 minor polities from a local, private comparison report.

The input CSV is produced by audit_aoc2_1440.py on a licensed local game
installation. No source geometry, scenario serialization or CSV enters git.
Most moves need at least 8 agreeing samples and 75% support. Cologne and
Ulster use documented unanimous small-sample exceptions; named-city
corrections use separate public historical sources. Bremen and Hamburg are
explicit city-cell gameplay approximations because their 1440 legal status is
disputed; game-reference votes do not determine their wider territory.
"""

from __future__ import annotations

import argparse
from collections import Counter, defaultdict
import csv
import hashlib
import json
from pathlib import Path


ROOT = Path(__file__).resolve().parents[2]
COUNTRIES = ROOT / "data/countries"
SCENARIO = ROOT / "data/scenarios/1440.json"
POLICY = ROOT / "data/map/europe/ownership_1440.json"
FLAGS = ROOT / "ui/src/assets/flags"
ASSETS = ROOT / "docs/ASSETS.md"

# Local game labels are leads for manual review, not historical border proof.
# The capital is the closest represented city where the historical seat is
# outside our raster or lies in a mixed-source cell.
POLITIES = {
    "bohe_m": ("boh", "Kingdom of Bohemia", "prague"),
    "perm_m": ("prm", "Principality of Perm", "berezniki"),
    "lio_m": ("liv", "Livonian Order", "tartu"),
    "ryaz_m": ("rya", "Principality of Ryazan", "ryazan"),
    "bava_m": ("bav", "Duchy of Bavaria", "munich"),
    "mus_m": ("mun", "Prince-Bishopric of Münster", "munster"),
    "sax_m": ("sax", "Electorate of Saxony", "leipzig"),
    "tyr_m": ("tyr", "Lordship of Tyrone", "omagh"),
    "sile_m": ("sil", "Duchies of Silesia", "wroc_aw"),
    "cann_m": ("can", "Beylik of Candar", "sinop"),
    "lorr_m": ("lor", "Duchy of Lorraine", "nancy"),
    "bn_m": ("brn", "Duchy of Brunswick", "braunschweig"),
    "prov_m": ("prv", "County of Provence", "aix_en_provence"),
    "wurz_m": ("wrz", "Prince-Bishopric of Würzburg", "wurzburg"),
    "yar_m": ("yar", "Principality of Yaroslavl", "yaroslavl"),
    "tver_m": ("tve", "Principality of Tver", "tver"),
    "dulk_m": ("dul", "Beylik of Dulkadir", "malatya"),
    "bit_m": ("bit", "Emirate of Bitlis", "van"),
    "belo_m": ("bel", "Principality of Beloozero", "cherepovets"),
    "pome_m": ("pom", "Duchy of Pomerania", "stralsund"),
    "magd_m": ("mag", "Archbishopric of Magdeburg", "magdeburg"),
}
THEOCRACIES = {"liv", "mun", "mag", "wrz"}
PUBLIC_CITY_CORRECTIONS = {
    "heidelberg": ("pal", "Electoral Palatinate", "monarchy"),
    "schaffhausen": ("shf", "Free Imperial City of Schaffhausen", "republic"),
}
SMALL_POLITIES = {
    "co_m": ("col", "Archbishopric of Cologne", "cologne", ("cologne",), "theocracy"),
    "uls_m": ("uls", "Ulster", "belfast", ("belfast", "lisburn"), "monarchy"),
}
CITY_STATE_APPROXIMATIONS = {
    # Their political/military standing around 1440 is disputed in the sources;
    # model only the named city cell as a playable municipality.
    "bremen": ("brm", "City of Bremen", "republic"),
    "hamburg": ("ham", "City of Hamburg", "republic"),
}


def read_json(path: Path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: Path, data):
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def flag_svg(tag: str, color: list[int]) -> str:
    digest = hashlib.sha256(tag.encode("ascii")).digest()
    base = "#" + "".join(f"{value:02x}" for value in color)
    accent = "#f2e9d5"
    motifs = (
        '<path d="M30 6v84M6 48h48" stroke="{a}" stroke-width="11"/>',
        '<path d="M-8 19 69 74M-8 75 69 20" stroke="{a}" stroke-width="10"/>',
        '<path d="M0 26h60M0 68h60" stroke="{a}" stroke-width="13"/>',
        '<path d="M30 12 54 48 30 84 6 48Z" fill="{a}"/>',
        '<circle cx="30" cy="48" r="21" fill="none" stroke="{a}" stroke-width="9"/>',
        '<path d="M0 0h30v96H0z" fill="{a}" opacity=".85"/>',
    )
    motif = motifs[digest[0] % len(motifs)].format(a=accent)
    return (f'<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 60 96" '
            f'role="img" aria-label="{tag} placeholder banner">'
            f'<rect width="60" height="96" fill="{base}"/>{motif}'
            '<path d="M2 2h56v92H2z" fill="none" stroke="#241f29" stroke-width="3"/>'
            '</svg>\n')


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("csv", type=Path, help="local AoC2 audit CSV")
    parser.add_argument("--apply", action="store_true")
    args = parser.parse_args()
    with args.csv.open(encoding="utf-8-sig", newline="") as stream:
        rows = list(csv.DictReader(stream))

    scenario = read_json(SCENARIO)
    countries = {tag: read_json(COUNTRIES / f"{tag}.json") for tag in scenario["countries"]}
    original_tags = set(countries)
    owner = {}
    for tag, country in countries.items():
        for key in country["start"]["provinces"]:
            if key in owner:
                raise ValueError(f"duplicate owner for {key}")
            owner[key] = tag
    protected_capitals = {country["capital"] for country in countries.values()}
    by_source: dict[str, list[str]] = defaultdict(list)
    seen = set()
    for row in rows:
        source = row["aoc2_tag"]
        if source not in POLITIES or float(row["confidence"]) < .75 or int(row["matched_samples"]) < 8:
            continue
        key = row["province_key"]
        if key in seen:
            raise ValueError(f"duplicate audit row {key}")
        seen.add(key)
        if key in protected_capitals and owner.get(key) != POLITIES[source][0]:
            continue
        if key not in owner:
            raise ValueError(f"missing current owner for {key}")
        # The steppe review is applied separately; do not rewrite its cells.
        if owner[key] in {"cri", "gth", "nog", "tim", "uzb"} and key != "ryazan":
            continue
        by_source[source].append(key)

    # This 1440 game reference votes Dresden into Bohemia, but the Wettin
    # electorate held it. Keep that named city with Saxony.
    if "dresden" in by_source["bohe_m"]:
        by_source["bohe_m"].remove("dresden")
        by_source["sax_m"].append("dresden")

    skipped = []
    moves = []
    new_countries = {}
    for source, (tag, name, capital) in POLITIES.items():
        if tag in countries:
            continue
        keys = sorted(set(by_source[source]))
        if capital not in keys:
            skipped.append(f"{name}: capital {capital} has no eligible sample")
            continue
        ordinal = len(new_countries)
        # Muted initial color. The adjacency-aware palette pass sets final colors.
        color = [118 + (ordinal * 43) % 86, 126 + (ordinal * 67) % 72,
                 118 + (ordinal * 59) % 84]
        new_countries[tag] = {
            "schema_version": 1, "tag": tag, "name": name, "color": color,
            "capital": capital, "ai_profile": "balanced",
            "government": "theocracy" if tag in THEOCRACIES else "monarchy",
            "start": {"ruler": None, "heir": None, "provinces": keys,
                      "treasury": 50000, "tax_rate": 220, "monthly_spending": 3500},
        }
        moves.extend((key, owner[key], tag) for key in keys)

    for key, (tag, name, government) in PUBLIC_CITY_CORRECTIONS.items():
        if tag in countries:
            continue
        if key in protected_capitals or key not in owner:
            raise ValueError(f"cannot add sourced city state {key}")
        ordinal = len(new_countries)
        color = [118 + (ordinal * 43) % 86, 126 + (ordinal * 67) % 72,
                 118 + (ordinal * 59) % 84]
        new_countries[tag] = {
            "schema_version": 1, "tag": tag, "name": name, "color": color,
            "capital": key, "ai_profile": "balanced", "government": government,
            "start": {"ruler": None, "heir": None, "provinces": [key],
                      "treasury": 50000, "tax_rate": 220, "monthly_spending": 3500},
        }
        moves.append((key, owner[key], tag))

    audit_by_key = {row["province_key"]: row for row in rows}
    for source, (tag, name, capital, keys, government) in SMALL_POLITIES.items():
        if tag in countries:
            continue
        if any(key in protected_capitals for key in keys):
            raise ValueError(f"cannot add small polity {tag}")
        for key in keys:
            row = audit_by_key.get(key)
            if (row is None or row["aoc2_tag"] != source
                    or float(row["confidence"]) < .95
                    or int(row["matched_samples"]) < 5 or key not in owner):
                raise ValueError(f"small polity {tag} lacks unanimous local support at {key}")
        ordinal = len(new_countries)
        color = [118 + (ordinal * 43) % 86, 126 + (ordinal * 67) % 72,
                 118 + (ordinal * 59) % 84]
        new_countries[tag] = {
            "schema_version": 1, "tag": tag, "name": name, "color": color,
            "capital": capital, "ai_profile": "balanced", "government": government,
            "start": {"ruler": None, "heir": None, "provinces": list(keys),
                      "treasury": 50000, "tax_rate": 220, "monthly_spending": 3500},
        }
        moves.extend((key, owner[key], tag) for key in keys)

    for key, (tag, name, government) in CITY_STATE_APPROXIMATIONS.items():
        if tag in countries:
            continue
        if key in protected_capitals or key not in owner:
            raise ValueError(f"cannot add city-state approximation {key}")
        ordinal = len(new_countries)
        color = [118 + (ordinal * 43) % 86, 126 + (ordinal * 67) % 72,
                 118 + (ordinal * 59) % 84]
        new_countries[tag] = {
            "schema_version": 1, "tag": tag, "name": name, "color": color,
            "capital": key, "ai_profile": "balanced", "government": government,
            "start": {"ruler": None, "heir": None, "provinces": [key],
                      "treasury": 50000, "tax_rate": 220, "monthly_spending": 3500},
        }
        moves.append((key, owner[key], tag))

    counts = Counter(target for _, _, target in moves)
    print(f"new countries={len(new_countries)}; moves={len(moves)}; skipped={len(skipped)}")
    for tag, count in sorted(counts.items()):
        print(f"{tag}: {count}")
    for note in skipped:
        print(f"skip: {note}")
    if not args.apply:
        print("dry run; pass --apply to write content")
        return

    for key, old, tag in moves:
        countries[old]["start"]["provinces"].remove(key)
        owner[key] = tag
    for tag, country in countries.items():
        if country["capital"] not in country["start"]["provinces"]:
            raise ValueError(f"{tag} lost capital")
    countries.update(new_countries)
    assert len(owner) == sum(len(c["start"]["provinces"]) for c in countries.values())
    policy = read_json(POLICY)
    overrides = policy["rules"]["explicit_province_tags"]
    for key, _, tag in moves:
        if key in overrides and overrides[key] != tag:
            raise ValueError(f"existing owner override conflicts for {key}")
        overrides[key] = tag
    scenario["countries"] = sorted(set(scenario["countries"]) | set(new_countries))
    for tag in sorted(original_tags):
        if any(old == tag for _, old, _ in moves):
            countries[tag]["start"]["provinces"].sort()
            write_json(COUNTRIES / f"{tag}.json", countries[tag])
    for tag, country in new_countries.items():
        write_json(COUNTRIES / f"{tag}.json", country)
        (FLAGS / f"{tag}.svg").write_text(flag_svg(tag, country["color"]), encoding="utf-8")
    write_json(SCENARIO, scenario)
    write_json(POLICY, policy)
    with ASSETS.open("a", encoding="utf-8") as stream:
        for tag in sorted(new_countries):
            stream.write(f'| `ui/src/assets/flags/{tag}.svg` | Local 1440 political-reference review; '
                         'abstract original placeholder, not a historical flag | Project team | Own work | '
                         'Original geometric banner for an AI minor country; replace only after sourced heraldic research. |\n')
    print("applied reviewed minor-country expansion")


if __name__ == "__main__":
    main()
