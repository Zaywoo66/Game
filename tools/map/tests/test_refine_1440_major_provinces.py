"""Tests deterministic major-only province subdivision and inheritance."""

import copy
from pathlib import Path
import sys
import unittest

import numpy as np

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from build_europe import PROJECTION  # noqa: E402
from map_common import map_hash  # noqa: E402
from refine_1440_major_provinces import refine  # noqa: E402


class MajorProvinceRefinementTests(unittest.TestCase):
    def setUp(self):
        self.ids = np.full((64, 64), 3, dtype=np.uint32)
        self.ids[0:40, 0:32] = 1
        self.ids[0:8, 40:48] = 2
        self.terrain = np.zeros((64, 64), dtype=np.uint8)
        self.terrain[self.ids == 1] = 1
        self.terrain[self.ids == 2] = 2
        self.rivers = np.zeros((64, 64), dtype=np.uint8)
        rows = [
            {"id": 1, "key": "england", "kind": "land", "terrain": "plains",
             "centroid": [20, 16], "neighbors": [], "ports": [], "development": 7,
             "name_key": "province_england", "area": "europe"},
            {"id": 2, "key": "frankfurt", "kind": "land", "terrain": "hills",
             "centroid": [44, 4], "neighbors": [], "ports": [], "development": 2,
             "name_key": "province_frankfurt", "area": "europe"},
            {"id": 3, "key": "sea", "kind": "sea", "terrain": "sea",
             "centroid": [60, 40], "neighbors": [], "ports": [], "development": 0,
             "name_key": "province_sea", "area": "water"},
        ]
        self.projection = dict(PROJECTION)
        self.digest = map_hash(rows, projection=self.projection)
        self.map_data = {"schema_version": 3, "map_version": 6, "map_hash": self.digest,
                         "projection": self.projection, "provinces": rows}
        self.config = {"input_map_version": 6, "input_map_hash": self.digest,
                       "output_map_version": 7, "target_physical_pixel_area": 100,
                       "refined_country_tags": ["eng"]}
        self.names = {"province_england": "England", "province_frankfurt": "Frankfurt",
                      "province_sea": "Sea", "area_europe": "Europe"}
        self.policy = {"map_version": 6, "map_hash": self.digest}

    def run_refine(self):
        return refine(
            copy.deepcopy(self.map_data), self.ids.copy(), self.terrain.copy(), self.rivers.copy(),
            {}, {"england": "eng", "frankfurt": "brm"}, self.names.copy(),
            self.config.copy(), self.policy.copy(),
        )

    def test_only_selected_owner_cells_split_and_keep_their_owner_parent(self):
        original_major = self.ids == 1
        major_map, result_ids, _, _, names, children = self.run_refine()
        self.assertGreater(len(children[1]), 1)
        self.assertNotIn(2, children)
        self.assertNotIn(3, children)
        self.assertEqual(set(np.unique(result_ids[original_major])), set(children[1]))
        self.assertTrue(np.array_equal(result_ids[self.ids == 2], self.ids[self.ids == 2]))
        self.assertEqual(major_map["map_version"], 7)
        self.assertEqual(len(names), len(set(names)))
        self.assertEqual(sum(row["development"] for row in major_map["provinces"] if row["key"].startswith("england")), 7)

    def test_refinement_is_deterministic(self):
        first = self.run_refine()
        second = self.run_refine()
        self.assertEqual(first[0]["map_hash"], second[0]["map_hash"])
        np.testing.assert_array_equal(first[1], second[1])
        self.assertEqual(first[4], second[4])


if __name__ == "__main__":
    unittest.main()
