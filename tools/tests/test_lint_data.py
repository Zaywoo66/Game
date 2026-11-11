from pathlib import Path
import subprocess
import sys
import unittest


class DataLintTest(unittest.TestCase):
    def test_rejects_invalid_json(self):
        test_dir = Path(__file__).resolve().parent
        result = subprocess.run(
            [
                sys.executable,
                str(test_dir.parent / "lint_data.py"),
                str(test_dir / "fixtures" / "bad_json"),
            ],
            capture_output=True,
            text=True,
            check=False,
        )
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("invalid.json: Expecting value", result.stderr)
