"""Regression checks for the complete Europe canvas and western seam."""

import json
import hashlib
from pathlib import Path
import sys
import unittest

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build_europe import OLD_WIDTH, OUTPUT, ROOT, WEST, WEST_Y, WEST_WIDTH, HEIGHT, WIDTH, decode_ids  # noqa: E402
from map_common import map_hash  # noqa: E402
from topology import raster_contacts  # noqa: E402


class EuropeMapTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.west = json.loads((WEST / "provinces.json").read_text())
        cls.europe = json.loads((OUTPUT / "provinces.json").read_text())
        cls.ids = decode_ids(OUTPUT / "provinces.png")

    def test_extent_and_stable_western_pixels(self):
        self.assertEqual(self.ids.shape, (HEIGHT, WIDTH))
        np.testing.assert_array_equal(self.ids[WEST_Y:, :WEST_WIDTH], decode_ids(WEST / "provinces.png"))
        by_id = {p["id"]: p for p in self.europe["provinces"]}
        for original in self.west["provinces"]:
            extended = by_id[original["id"]]
            self.assertEqual(extended["key"], original["key"])
            self.assertEqual(extended["kind"], original["kind"])
            self.assertEqual(extended["centroid"], [original["centroid"][0], original["centroid"][1] + WEST_Y])

    def test_previous_europe_pixels_and_ids_are_preserved(self):
        rgb = np.asarray(Image.open(OUTPUT / "provinces.png").convert("RGB"))
        prefix = rgb[:, :OLD_WIDTH]
        self.assertEqual(
            hashlib.sha256(prefix.tobytes()).hexdigest(),
            "9c39f409e0df6d136a368765db9478ddb3c4c1d7d950e7ef434630681eea9666",
        )
        provinces = json.loads((OUTPUT / "provinces.json").read_text())["provinces"]
        self.assertEqual([row["id"] for row in provinces[:1332]], list(range(1, 1333)))

    def test_all_pixels_and_seam_have_graph_edges(self):
        provinces = {p["id"]: p for p in self.europe["provinces"]}
        self.assertNotIn(0, np.unique(self.ids))
        self.assertEqual(set(np.unique(self.ids)), set(provinces))
        self.assertLess(max(provinces), 2000)
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
        self.assertEqual(self.europe["map_version"], 6)
        tura = next(row for row in self.europe["provinces"] if row["key"] == "tura")
        self.assertEqual(tura["kind"], "land")

    def test_europe_scenario_accounts_for_every_new_land_province(self):
        scenario = json.loads((ROOT / "data/scenarios/1440.json").read_text())
        western = {p["id"] for p in self.west["provinces"]}
        new_land = {p["key"] for p in self.europe["provinces"]
                    if p["kind"] == "land" and p["id"] not in western}
        owned = set()
        for country in scenario["countries"]:
            record = json.loads((ROOT / "data/countries" / f"{country}.json").read_text())
            owned.update(record["start"]["provinces"])
        unowned = set(scenario["unowned_provinces"])
        self.assertTrue(new_land <= owned | unowned)
        self.assertFalse(owned & unowned)
        self.assertEqual(len(scenario["countries"]), len(set(scenario["countries"])))


if __name__ == "__main__":
    unittest.main()
