"""Unit tests for the bulk historical ownership seeding pass."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from assign_1440_ownership import majority_owner_updates, nearest_owner_updates  # noqa: E402


def row(key, polity, overlap, total=100, resolution="polygon"):
    return {
        "province_key": key,
        "polity_name": polity,
        "overlap_pixels": overlap,
        "province_pixels": total,
        "resolution": resolution,
    }


def fixtures():
    scenario = {"unowned_provinces": ["majority", "boundary", "coarse", "unknown", "owned"]}
    countries = {
        "bur": {"name": "Duchy of Burgundy", "start": {"provinces": ["owned"]}},
    }
    return scenario, countries


class OwnershipAssignmentTests(unittest.TestCase):
    def test_assigns_majority_named_and_alias_polygons(self):
        scenario, countries = fixtures()
        updated_scenario, updated_countries, counts = majority_owner_updates(
            scenario,
            countries,
            [row("majority", "Duchy of Burgundy", 50), row("boundary", "Burgundian Netherlands", 80)],
        )
        self.assertEqual(updated_countries["bur"]["start"]["provinces"], ["boundary", "majority", "owned"])
        self.assertEqual(updated_scenario["unowned_provinces"], ["coarse", "unknown", "owned"])
        self.assertEqual(counts["bur"], 2)

    def test_assigns_habsburg_source_to_austria_alias(self):
        scenario = {"unowned_provinces": ["vienna"]}
        countries = {"aus": {"name": "Archduchy of Austria", "start": {"provinces": []}}}
        updated_scenario, updated_countries, counts = majority_owner_updates(
            scenario, countries, [row("vienna", "House of Habsburg", 84)]
        )
        self.assertEqual(updated_scenario["unowned_provinces"], [])
        self.assertEqual(updated_countries["aus"]["start"]["provinces"], ["vienna"])
        self.assertEqual(counts["aus"], 1)

    def test_ignores_partial_coarse_unmatched_and_unrepresented_cells(self):
        scenario, countries = fixtures()
        updated_scenario, updated_countries, counts = majority_owner_updates(
            scenario,
            countries,
            [
                row("boundary", "Duchy of Burgundy", 49),
                row("coarse", "Duchy of Burgundy", 100, resolution="coarse_group"),
                row("unknown", None, 0, resolution="unmatched"),
                row("majority", "Kingdom of Poland", 100),
            ],
        )
        self.assertEqual(updated_scenario, scenario)
        self.assertEqual(updated_countries, countries)
        self.assertEqual(counts, {})

    def test_explicit_lower_threshold_can_fill_clear_known_country_edges(self):
        scenario, countries = fixtures()
        updated_scenario, updated_countries, counts = majority_owner_updates(
            scenario,
            countries,
            [row("boundary", "Burgundian Netherlands", 35)],
            threshold=0.3,
        )
        self.assertEqual(updated_scenario["unowned_provinces"], ["majority", "coarse", "unknown", "owned"])
        self.assertEqual(updated_countries["bur"]["start"]["provinces"], ["boundary", "owned"])
        self.assertEqual(counts["bur"], 1)

    def test_never_overwrites_an_existing_owner_and_validates_threshold(self):
        scenario, countries = fixtures()
        updated_scenario, updated_countries, counts = majority_owner_updates(
            scenario, countries, [row("owned", "Burgundian Netherlands", 100)]
        )
        self.assertEqual(updated_scenario, scenario)
        self.assertEqual(updated_countries, countries)
        self.assertEqual(counts, {})
        with self.assertRaises(ValueError):
            majority_owner_updates(scenario, countries, [], threshold=0.29)

    def test_nearest_fallback_fills_all_cells_and_keeps_hre_split_between_members(self):
        scenario = {"unowned_provinces": ["named", "gap", "hre"]}
        countries = {
            "a": {"tag": "a", "name": "Country A", "capital": "pa", "start": {"provinces": ["pa"]}},
            "b": {"tag": "b", "name": "Country B", "capital": "pb", "start": {"provinces": ["pb"]}},
        }
        map_data = {"provinces": [
            {"id": 1, "key": "pa", "neighbors": [{"id": 2}, {"id": 4}]},
            {"id": 2, "key": "named", "neighbors": [{"id": 1}, {"id": 3}]},
            {"id": 3, "key": "pb", "neighbors": [{"id": 2}, {"id": 4}, {"id": 5}]},
            {"id": 4, "key": "gap", "neighbors": [{"id": 1}, {"id": 3}]},
            {"id": 5, "key": "hre", "neighbors": [{"id": 3}]},
        ]}
        evidence = {"assignments": [
            row("named", "Country A", 5, total=100),
            row("gap", None, 0, resolution="unmatched"),
            row("hre", "Holy Roman Empire Minor States", 0, resolution="coarse_group"),
        ]}

        updated_scenario, updated_countries, named, fallback = nearest_owner_updates(
            scenario, countries, map_data, evidence, {"fallback": {"hre_owner_tags": ["a"]}}
        )

        self.assertEqual(updated_scenario["unowned_provinces"], [])
        self.assertEqual(updated_countries["a"]["start"]["provinces"], ["gap", "hre", "named", "pa"])
        self.assertEqual(updated_countries["b"]["start"]["provinces"], ["pb"])
        self.assertEqual(named["a"], 1)
        self.assertEqual(fallback["a"], 1)
        self.assertEqual(fallback["hre:a"], 1)

    def test_hre_fallback_reassigns_cells_away_from_non_member_owner(self):
        scenario = {"unowned_provinces": []}
        countries = {
            "a": {"tag": "a", "name": "Member A", "capital": "a_home",
                  "start": {"provinces": ["a_home"]}},
            "flo": {"tag": "flo", "name": "Florence", "capital": "flo_home",
                    "start": {"provinces": ["flo_home", "hre_cell"]}},
        }
        map_data = {"provinces": [
            {"id": 1, "key": "a_home", "neighbors": [{"id": 2}]},
            {"id": 2, "key": "hre_cell", "neighbors": [{"id": 1}, {"id": 3}]},
            {"id": 3, "key": "flo_home", "neighbors": [{"id": 2}]},
        ]}
        evidence = {"assignments": [
            row("hre_cell", "Holy Roman Empire Minor States", 0, resolution="coarse_group")
        ]}

        _, updated_countries, _, fallback = nearest_owner_updates(
            scenario, countries, map_data, evidence, {"fallback": {"hre_owner_tags": ["a"]}}
        )

        self.assertIn("hre_cell", updated_countries["a"]["start"]["provinces"])
        self.assertNotIn("hre_cell", updated_countries["flo"]["start"]["provinces"])
        self.assertEqual(fallback["hre:a"], 1)

    def test_hre_fallback_preserves_explicitly_reviewed_minor_owner(self):
        scenario = {"unowned_provinces": []}
        countries = {
            "a": {"tag": "a", "name": "Member A", "capital": "a_home",
                  "start": {"provinces": ["a_home"]}},
            "b": {"tag": "b", "name": "Minor B", "capital": "hre_cell",
                  "start": {"provinces": ["hre_cell"]}},
        }
        map_data = {"provinces": [
            {"id": 1, "key": "a_home", "neighbors": [{"id": 2}]},
            {"id": 2, "key": "hre_cell", "neighbors": [{"id": 1}]},
        ]}
        evidence = {"assignments": [
            row("hre_cell", "Holy Roman Empire Minor States", 0, resolution="coarse_group")
        ]}
        _, updated, _, fallback = nearest_owner_updates(
            scenario, countries, map_data, evidence,
            {"fallback": {"hre_owner_tags": ["a"]},
             "explicit_province_tags": {"hre_cell": "b"}},
        )
        self.assertEqual(updated["b"]["start"]["provinces"], ["hre_cell"])
        self.assertEqual(fallback, {})


if __name__ == "__main__":
    unittest.main()
