import sys
from pathlib import Path
import unittest

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from apply_aoc2_steppe_review import apply_moves, plan_reassignments  # noqa: E402


class SteppeReviewTests(unittest.TestCase):
    def setUp(self):
        self.map_data = {"provinces": [
            {"id": 1, "key": "capital", "kind": "land"},
            {"id": 2, "key": "border", "kind": "land"},
            {"id": 3, "key": "sea", "kind": "sea"},
        ]}
        self.countries = {
            "gth": {"capital": "capital", "start": {"provinces": ["capital", "border"]}},
            "nog": {"capital": "nog_cap", "start": {"provinces": ["nog_cap"]}},
            "uzb": {"capital": "uzb_cap", "start": {"provinces": ["uzb_cap"]}},
            "cri": {"capital": "cri_cap", "start": {"provinces": ["cri_cap"]}},
        }

    def row(self, **changes):
        row = {
            "province_id": "2", "province_key": "border",
            "current_owner": "gth", "aoc2_tag": "nago_h",
            "confidence": "0.875", "matched_samples": "14",
            "total_samples": "16", "other_tags": "",
        }
        row.update(changes)
        return row

    def test_high_confidence_reassigns_and_preserves_capital(self):
        moves, counts = plan_reassignments(self.map_data, self.countries, [self.row()])
        self.assertEqual(moves, [("border", "gth", "nog")])
        self.assertEqual(counts["eligible_moves"], 1)
        affected = apply_moves(self.countries, moves)
        self.assertEqual(affected, {"gth", "nog"})
        self.assertEqual(self.countries["gth"]["start"]["provinces"], ["capital"])
        self.assertIn("border", self.countries["nog"]["start"]["provinces"])

    def test_low_confidence_and_sparse_samples_are_not_applied(self):
        for row in (self.row(confidence="0.749"), self.row(matched_samples="7")):
            moves, counts = plan_reassignments(self.map_data, self.countries, [row])
            self.assertEqual(moves, [])
            self.assertEqual(counts["low_confidence"], 1)

    def test_capital_cannot_be_moved(self):
        moves, counts = plan_reassignments(self.map_data, self.countries, [
            self.row(province_id="1", province_key="capital")
        ])
        self.assertEqual(moves, [])
        self.assertEqual(counts["capital_protected"], 1)

    def test_stale_or_duplicate_owner_fails(self):
        with self.assertRaisesRegex(ValueError, "stale/unmatched"):
            plan_reassignments(self.map_data, self.countries, [
                self.row(current_owner="uzb")
            ])
        self.countries["nog"]["start"]["provinces"].append("border")
        with self.assertRaisesRegex(ValueError, "multiple current owners"):
            plan_reassignments(self.map_data, self.countries, [self.row()])

    def test_unknown_province_or_id_mismatch_fails(self):
        with self.assertRaisesRegex(ValueError, "unknown/non-land"):
            plan_reassignments(self.map_data, self.countries, [
                self.row(province_key="sea", province_id="3")
            ])
        with self.assertRaisesRegex(ValueError, "ID mismatch"):
            plan_reassignments(self.map_data, self.countries, [
                self.row(province_id="99")
            ])


if __name__ == "__main__":
    unittest.main()
