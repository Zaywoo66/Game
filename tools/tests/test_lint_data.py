from pathlib import Path
import subprocess
import sys
import unittest


class DataLintTest(unittest.TestCase):
    def test_rejects_invalid_json(self):
        result = run_lint("bad_json")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("invalid.json: Expecting value", result.stderr)

    def test_rejects_non_integer_and_out_of_range_fixed_values(self):
        result = run_lint("bad_fixed")
        self.assertEqual(result.returncode, 1, result)
        invalid_paths = [
            "invalid.json: $.options[0].effects[0].add_modifier.value",
            *(f"invalid.json: $.modifiers[{index}].value" for index in range(6)),
            "invalid.yaml: $.effects[0].add_modifier.value",
            "invalid.yaml: $.effects[1].add_modifier.value",
            *(f"invalid.yml: $.modifiers[{index}].value" for index in range(3)),
        ]
        errors = result.stderr.splitlines()
        self.assertEqual(len(errors), len(invalid_paths), result.stderr)
        for path in invalid_paths:
            with self.subTest(path=path):
                self.assertIn(
                    f"{path}: expected a raw Fixed integer in the signed 64-bit range",
                    errors,
                )

    def test_rejects_invalid_plain_integer_durations(self):
        result = run_lint("bad_modifier_duration")
        self.assertEqual(result.returncode, 1, result)
        errors = result.stderr.splitlines()
        self.assertEqual(len(errors), 6, result.stderr)
        for index in range(6):
            with self.subTest(index=index):
                self.assertIn(
                    f"invalid.yaml: $.effects[{index}].add_modifier.duration_days: "
                    "expected a positive integer day count in the unsigned 64-bit range",
                    errors,
                )

    def test_accepts_raw_integers_and_ignores_unrelated_values(self):
        result = run_lint("good_fixed")
        self.assertEqual(result.returncode, 0, result)
        self.assertEqual(result.stderr, "")
        self.assertIn("Checked 3 JSON and YAML data files", result.stdout)

    def test_accepts_event_contract_with_conditional_ai_weight(self):
        result = run_lint("good_event")
        self.assertEqual(result.returncode, 0, result)
        self.assertEqual(result.stderr, "")

    def test_rejects_unknown_trigger_effect_missing_weight_and_fractional_mtth(self):
        result = run_lint("bad_event")
        self.assertEqual(result.returncode, 1, result)
        for field in (
            "$.trigger.unknown_trigger: unknown trigger",
            "$.mean_time_to_happen: expected an integer",
            "$.options[0].ai_weight: required field",
            "$.options[0].effects[0].unknown_effect: unknown effect",
        ):
            with self.subTest(field=field):
                self.assertIn(field, result.stderr)

    def test_rejects_noninteger_ai_weights_and_cooldown(self):
        result = run_lint("bad_event_weights")
        self.assertEqual(result.returncode, 1, result)
        for field in (
            "$.cooldown_days: expected an integer",
            "$.options[0].ai_weight: expected an integer",
            "$.options[0].ai_weight_if.add: expected an integer",
        ):
            with self.subTest(field=field):
                self.assertIn(field, result.stderr)

    def test_rejects_duplicate_event_keys(self):
        result = run_lint("duplicate_events")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("second.yaml: $.key: duplicate event key also in", result.stderr)
        self.assertIn("first.yaml", result.stderr)

    def test_rejects_empty_snake_case_segments(self):
        result = run_lint("bad_event_key")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("$.key: expected an ASCII snake_case", result.stderr)


def run_lint(fixture):
    test_dir = Path(__file__).resolve().parent
    return subprocess.run(
        [
            sys.executable,
            str(test_dir.parent / "lint_data.py"),
            str(test_dir / "fixtures" / fixture),
        ],
        capture_output=True,
        text=True,
        check=False,
        timeout=15,
    )
