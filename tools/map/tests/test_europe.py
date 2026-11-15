"""Regression checks for the complete Europe canvas and western seam."""

import json
from pathlib import Path
import sys
import unittest
from zipfile import ZipFile

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build_europe import BASELINE, OUTPUT, ROOT, WEST, HEIGHT, WIDTH, decode_ids  # noqa: E402
from map_common import map_hash  # noqa: E402
from topology import raster_contacts  # noqa: E402


class EuropeMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.west = json.loads((WEST / "provinces.json").read_text())
        cls.europe = json.loads((OUTPUT / "provinces.json").read_text())
        cls.ids = decode_ids(OUTPUT / "provinces.png")

    def test_extent_and_stable_western_province_ids(self):
        self.assertEqual(self.ids.shape, (HEIGHT, WIDTH))
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
        self.assertLess(max(provinces), 4000)
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
        self.assertEqual(self.europe["map_version"], 7)
        land_count = sum(row["kind"] == "land" for row in self.europe["provinces"])
        self.assertGreaterEqual(land_count, 2800)
        self.assertLessEqual(land_count, 3300)
        tura = next(row for row in self.europe["provinces"] if row["key"] == "tura")
        self.assertEqual(tura["kind"], "land")

    def test_europe_scenario_accounts_for_owned_and_colonizable_land(self):
        scenario = json.loads((ROOT / "data/scenarios/1440.json").read_text())
        owned = set()
        owner_by_key = {}
        for country in scenario["countries"]:
            record = json.loads((ROOT / "data/countries" / f"{country}.json").read_text())
            owned.update(record["start"]["provinces"])
            owner_by_key.update({key: country for key in record["start"]["provinces"]})
        unowned = set(scenario["unowned_provinces"])
        land_keys = {row["key"] for row in self.europe["provinces"] if row["kind"] == "land"}
        self.assertEqual(owned | unowned, land_keys)
        self.assertFalse(owned & unowned)
        rules = json.loads((ROOT / "data/map/europe/ownership_1440.json").read_text())["rules"]
        frontier_tags = set(rules["northern_russia_frontier"]["owner_tags"])
        by_key = {row["key"]: row for row in self.europe["provinces"]}
        self.assertEqual(len(unowned), 33)
        for key in unowned:
            row = by_key[key]
            x, y = row["centroid"]
            latitude = 72.0 - (y + 0.5) / 32.0
            longitude = -13.0 + (x + 0.5) / 32.0
            self.assertGreaterEqual(latitude, 68.0)
            self.assertGreaterEqual(longitude, 30.0)
        for key, owner in owner_by_key.items():
            x, y = by_key[key]["centroid"]
            latitude = 72.0 - (y + 0.5) / 32.0
            longitude = -13.0 + (x + 0.5) / 32.0
            self.assertFalse(owner in frontier_tags and latitude >= 68.0 and longitude >= 30.0)
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
