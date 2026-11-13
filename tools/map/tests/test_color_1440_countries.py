"""Unit tests for neighbor-aware alpha-map country colors."""

from pathlib import Path
import sys
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))
from color_1440_countries import assign_colors  # noqa: E402


class CountryColorTests(unittest.TestCase):
    def test_adjacent_country_colors_are_distinct_and_input_is_unchanged(self):
        scenario = {"countries": ["a", "b", "c"]}
        countries = {
            "a": {"color": [1, 2, 3], "start": {"provinces": ["pa"]}},
            "b": {"color": [1, 2, 3], "start": {"provinces": ["pb"]}},
            "c": {"color": [1, 2, 3], "start": {"provinces": ["pc"]}},
        }
        map_data = {"provinces": [
            {"id": 1, "key": "pa", "neighbors": [{"id": 2}]},
            {"id": 2, "key": "pb", "neighbors": [{"id": 1}, {"id": 3}]},
            {"id": 3, "key": "pc", "neighbors": [{"id": 2}]},
        ]}

        updated, _ = assign_colors(scenario, countries, map_data)

        self.assertNotEqual(updated["a"]["color"], updated["b"]["color"])
        self.assertNotEqual(updated["b"]["color"], updated["c"]["color"])
        self.assertEqual(countries["a"]["color"], [1, 2, 3])

    def test_palette_scales_past_initial_twelve_colors(self):
        count = 26
        scenario = {"countries": [f"c{index:02}" for index in range(count)]}
        countries = {
            tag: {"color": [1, 2, 3], "start": {"provinces": [f"p{index}"]}}
            for index, tag in enumerate(scenario["countries"])
        }
        provinces = []
        for index in range(count):
            neighbors = []
            if index:
                neighbors.append({"id": index})
            if index + 1 < count:
                neighbors.append({"id": index + 2})
            provinces.append({"id": index + 1, "key": f"p{index}", "neighbors": neighbors})

        updated, _ = assign_colors(scenario, countries, {"provinces": provinces})

        self.assertEqual(len({tuple(country["color"]) for country in updated.values()}), count)


if __name__ == "__main__":
    unittest.main()
