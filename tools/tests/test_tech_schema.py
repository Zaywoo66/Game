import sys
from pathlib import Path
import unittest


sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from tech_schema import validate_prerequisites


class TechGraphTest(unittest.TestCase):
    def test_valid_cross_branch_dag(self):
        graph = {
            "military_drill": ["administrative_recordkeeping"],
            "administrative_recordkeeping": [],
            "professional_officers": ["military_drill"],
        }
        self.assertEqual(validate_prerequisites(graph), [])

    def test_missing_prerequisite(self):
        self.assertEqual(
            validate_prerequisites({"military_drill": ["unknown_tech"]}),
            ["military_drill: missing prerequisite unknown_tech"],
        )

    def test_self_cycle(self):
        self.assertEqual(
            validate_prerequisites({"military_drill": ["military_drill"]}),
            ["cycle: military_drill -> military_drill"],
        )

    def test_cross_branch_cycle_reported_once(self):
        graph = {
            "military_drill": ["administrative_recordkeeping"],
            "administrative_recordkeeping": ["professional_officers"],
            "professional_officers": ["military_drill"],
        }
        self.assertEqual(
            validate_prerequisites(graph),
            ["cycle: administrative_recordkeeping -> professional_officers -> military_drill -> administrative_recordkeeping"],
        )


if __name__ == "__main__":
    unittest.main()
