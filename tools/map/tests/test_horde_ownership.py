"""Checks the owner-approved approximate 1440 steppe partition."""

import json
from pathlib import Path
import sys
import unittest

ROOT = Path(__file__).resolve().parents[3]
sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from assign_1440_ownership import owner_directed_assignments  # noqa: E402


class HordeOwnershipTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.map_data = json.loads((ROOT / "data/map/europe/provinces.json").read_text())
        cls.evidence = json.loads((ROOT / "data/map/europe/polities_1440.json").read_text())
        cls.policy = json.loads((ROOT / "data/map/europe/ownership_1440.json").read_text())
        cls.owners = owner_directed_assignments(cls.map_data, cls.evidence, cls.policy)

    def test_policy_is_pinned_to_the_current_map(self):
        identity = self.map_data["map_version"], self.map_data["map_hash"]
        self.assertEqual(identity, (self.evidence["map_version"], self.evidence["map_hash"]))
        self.assertEqual(identity, (self.policy["map_version"], self.policy["map_hash"]))

    def test_user_approved_regions_have_expected_owners(self):
        rules = self.policy["rules"]
        self.assertEqual(self.owners["tura"], "uzb")
        self.assertEqual(self.owners["astrakhan"], "gth")
        self.assertEqual(self.owners["atyrau"], "nog")
        self.assertEqual(self.owners["sevastopol"], "cri")
        self.assertEqual(self.owners["kerch"], "cri")
        self.assertEqual(self.owners["dzhankoy"], "cri")
        for key in rules["north_caucasus_keys"]:
            self.assertEqual(self.owners[key], "gth")

    def test_legacy_golden_horde_label_resolves_to_great_horde_nogai_and_uzbek_bands(self):
        provinces = {row["key"]: row for row in self.map_data["provinces"]}
        evidence = {row["province_key"]: row for row in self.evidence["assignments"]}
        rules = self.policy["rules"]
        for key, row in evidence.items():
            if row["polity_name"] != "Golden Horde" or key in self.policy["rules"]["north_caucasus_keys"]:
                continue
            if key in rules["capital_proxy_tags"]:
                self.assertEqual(self.owners[key], rules["capital_proxy_tags"][key])
                continue
            if key in rules["explicit_province_tags"]:
                self.assertEqual(self.owners[key], rules["explicit_province_tags"][key])
                continue
            x = provinces[key]["centroid"][0]
            longitude = -13.0 + (x + 0.5) / 32.0
            if longitude < rules["middle_west_longitude_degrees"]:
                expected = "gth"
            elif longitude < rules["eastern_longitude_degrees"]:
                expected = "nog"
            else:
                expected = "uzb"
            self.assertEqual(self.owners[key], expected, key)
        self.assertEqual(rules["legacy_map_label"].split(";")[0],
                         "Golden Horde is a legacy regional label")

    def test_kalmar_coarse_cells_split_into_three_independent_owners(self):
        rules = self.policy["rules"]
        kalmar_rows = [row for row in self.evidence["assignments"]
                       if row["polity_name"] == rules["kalmar_source_name"]
                       and row["resolution"] == "coarse_group"]
        self.assertEqual(len(kalmar_rows), 139)
        assigned = {self.owners[row["province_key"]] for row in kalmar_rows}
        self.assertEqual(assigned, {"den", "nor", "swe"})
        self.assertEqual(self.owners["k_benhavn"], "den")
        self.assertEqual(self.owners["oslo"], "nor")
        self.assertEqual(self.owners["stockholm"], "swe")

    def test_nordic_countries_have_scenario_rulers_and_banners(self):
        scenario = json.loads((ROOT / "data/scenarios/1440.json").read_text())
        for tag, ruler in (("den", "christopher_iii"), ("nor", "erik_of_pomerania"),
                           ("swe", "karl_knutsson")):
            country = json.loads((ROOT / f"data/countries/{tag}.json").read_text())
            self.assertIn(tag, scenario["countries"])
            self.assertEqual(country["start"]["ruler"], ruler)
            self.assertIn(country["capital"], country["start"]["provinces"])
            self.assertTrue((ROOT / f"ui/src/assets/flags/{tag}.svg").is_file())

    def test_steppe_countries_have_campaign_rulers_and_capitals(self):
        scenario = json.loads((ROOT / "data/scenarios/1440.json").read_text())
        countries = {tag: json.loads((ROOT / f"data/countries/{tag}.json").read_text())
                     for tag in ("gth", "nog", "uzb", "cri")}
        characters = {key: json.loads((ROOT / f"data/characters/{key}.json").read_text())
                       for key in ("sayyid_ahmad", "wakkas_biy", "abul_khayr", "haci_giray")}
        for tag, country in countries.items():
            self.assertIn(tag, scenario["countries"])
            self.assertIn(country["capital"], country["start"]["provinces"])
            self.assertIn(country["start"]["ruler"], scenario["characters"])
        self.assertEqual(characters["sayyid_ahmad"]["birth"], [1400, 1, 1])
        self.assertEqual(characters["wakkas_biy"]["birth"], [1400, 1, 1])


if __name__ == "__main__":
    unittest.main()
