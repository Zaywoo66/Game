"""Regression checks for the complete Europe canvas and western seam."""

import json
from pathlib import Path
import sys
import unittest
from zipfile import ZipFile

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build_europe import BASELINE, OUTPUT, ROOT, WEST, decode_ids  # noqa: E402
from map_common import map_hash  # noqa: E402
from topology import raster_contacts  # noqa: E402


class EuropeMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.west = json.loads((WEST / "provinces.json").read_text())
        cls.europe = json.loads((OUTPUT / "provinces.json").read_text())
        cls.ids = decode_ids(OUTPUT / "provinces.png")

    def test_extent_and_stable_western_province_ids(self):
        political_fill = np.asarray(Image.open(OUTPUT / "country_fill_2022.png"))
        political_borders = np.asarray(Image.open(OUTPUT / "country_borders_2022.png"))
        self.assertEqual(self.ids.shape, political_fill.shape[:2])
        self.assertEqual(self.ids.shape, political_borders.shape[:2])
        by_id = {p["id"]: p for p in self.europe["provinces"]}
        for original in self.west["provinces"]:
            extended = by_id[original["id"]]
            self.assertEqual(extended["key"], original["key"])
            self.assertEqual(extended["kind"], original["kind"])
            x, y = extended["centroid"]
            self.assertEqual(int(self.ids[y, x]), original["id"])

    def test_previous_europe_province_ids_and_keys_are_preserved(self):
        with ZipFile(BASELINE) as archive:
            baseline = json.loads(archive.read("provinces.json"))
        provinces = {row["id"]: row for row in self.europe["provinces"]}
        for original in baseline["provinces"]:
            self.assertEqual(provinces[original["id"]]["key"], original["key"])
            self.assertEqual(provinces[original["id"]]["kind"], original["kind"])

    def test_all_pixels_and_seam_have_graph_edges(self):
        provinces = {p["id"]: p for p in self.europe["provinces"]}
        self.assertNotIn(0, np.unique(self.ids))
        self.assertEqual(set(np.unique(self.ids)), set(provinces))
        self.assertLess(max(provinces), 5000)
        river = np.asarray(Image.open(OUTPUT / "rivers.png")) != 0
        contacts, _ = raster_contacts(self.ids, river)
        western_ids = {p["id"] for p in self.west["provinces"]}
        seam = [pair for pair in contacts if (pair[0] in western_ids) != (pair[1] in western_ids)]
        self.assertTrue(seam)
        for a, b in seam:
            self.assertTrue(any(e["id"] == b for e in provinces[a]["neighbors"]))
            self.assertTrue(any(e["id"] == a for e in provinces[b]["neighbors"]))

    def test_hash_evidence_and_coverage(self):
        self.assertEqual(self.europe["map_hash"], map_hash(self.europe["provinces"], projection=self.europe["projection"]))
        evidence = json.loads((OUTPUT / "polities_1440.json").read_text())
        self.assertEqual(evidence["map_hash"], self.europe["map_hash"])
        land_ids = {p["id"] for p in self.europe["provinces"] if p["kind"] == "land"}
        self.assertEqual({item["province_id"] for item in evidence["assignments"]}, land_ids)
        self.assertTrue(all(item["resolution"] in {"polygon", "coarse_group", "unmatched"}
                            for item in evidence["assignments"]))
        self.assertEqual(evidence["source"]["release"], "v0.2.1")
        self.assertIn("documented Bavaria-to-Burgundian-Netherlands correction", evidence["method"])
        polity_by_key = {item["province_key"]: item["polity_name"] for item in evidence["assignments"]}
        self.assertEqual(polity_by_key["amsterdam"], "Burgundian Netherlands")
        self.assertEqual(polity_by_key["munich"], "Duchy of Bavaria")
        self.assertEqual(self.europe["map_version"], 9)
        land_count = sum(row["kind"] == "land" for row in self.europe["provinces"])
        self.assertGreaterEqual(land_count, 2800)
        self.assertLessEqual(land_count, 3600)
        tura = next(row for row in self.europe["provinces"] if row["key"] == "tura")
        self.assertEqual(tura["kind"], "land")

    def test_2022_scenario_owns_every_land_province(self):
        scenario = json.loads((ROOT / "data/scenarios/2022.json").read_text())
        ownership = json.loads((ROOT / "data/map/europe/ownership_2022.json").read_text())
        self.assertEqual(ownership["map_version"], self.europe["map_version"])
        self.assertEqual(ownership["map_hash"], self.europe["map_hash"])
        owner_by_key = {}
        for country in scenario["countries"]:
            record = json.loads((ROOT / "data/countries" / f"{country}.json").read_text())
            for key in record["start"]["provinces"]:
                self.assertNotIn(key, owner_by_key, f"duplicate 2022 owner for {key}")
                owner_by_key[key] = country
        land_keys = {row["key"] for row in self.europe["provinces"] if row["kind"] == "land"}
        self.assertEqual(set(scenario["unowned_provinces"]), set())
        self.assertEqual(set(owner_by_key), land_keys)
        provinces_by_id = {row["id"]: row for row in self.europe["provinces"]}
        expected = {
            provinces_by_id[int(province_id)]["key"]: tag
            for province_id, tag in ownership["province_owners"].items()
        }
        self.assertEqual(owner_by_key, expected)
        self.assertEqual(len(scenario["countries"]), len(set(scenario["countries"])))

    def test_refinement_only_splits_owner_approved_major_countries(self):
        config = json.loads((ROOT / "tools/map/inputs/europe/major_density_1440.json").read_text())
        owners = {}
        for tag in json.loads((ROOT / "data/scenarios/1440.json").read_text())["countries"]:
            country = json.loads((ROOT / f"data/countries/{tag}.json").read_text())
            for key in country["start"]["provinces"]:
                owners[key] = tag
        child_parent = {
            row["key"]: row["key"].rsplit("_part_", 1)[0]
            for row in self.europe["provinces"] if "_part_" in row["key"]
        }
        unowned = set(json.loads((ROOT / "data/scenarios/1440.json").read_text())["unowned_provinces"])
        self.assertTrue(child_parent)
        for child, parent in child_parent.items():
            if child in unowned:
                continue
            self.assertIn(owners[child], config["refined_country_tags"])
            if parent in owners:
                self.assertEqual(owners[child], owners[parent])


if __name__ == "__main__":
    unittest.main()
