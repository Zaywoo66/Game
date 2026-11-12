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

    def test_rejects_missing_or_empty_event_localization(self):
        result = run_lint("bad_event_locale")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("missing English event localization key event_notice_desc", result.stderr)
        self.assertIn("event_notice_ack: expected nonempty English text", result.stderr)

    def test_rejects_duplicate_event_localization_key(self):
        result = run_lint("bad_event_locale_duplicate")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("duplicate event localization key event_notice_title", result.stderr)

    def test_rejects_missing_translated_event_localization_key(self):
        result = run_lint("bad_event_locale_ru")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("missing RU event localization key event_notice_ack", result.stderr)

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

    def test_rejects_yaml_11_boolean_aliases_in_events(self):
        result = run_lint("bad_event_bool")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("boolean token 'yes' must be lowercase true or false", result.stderr)
        self.assertIn("boolean token 'TRUE' must be lowercase true or false", result.stderr)

    def test_rejects_noncanonical_event_integer_tokens(self):
        result = run_lint("bad_event_numbers")
        self.assertEqual(result.returncode, 1, result)
        for token in ("'012'", "'1_000'", "'+10'", "'-01'", "'0x10'"):
            with self.subTest(token=token):
                self.assertIn(f"integer token {token} must be plain decimal", result.stderr)

    def test_rejects_duplicate_yaml_mapping_fields(self):
        result = run_lint("bad_event_duplicate_field")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("duplicate YAML mapping key 'ai_weight'", result.stderr)

    def test_accepts_cross_branch_technology_dag_and_fixed_effect(self):
        result = run_lint("good_tech")
        self.assertEqual(result.returncode, 0, result.stderr)
        self.assertEqual(result.stderr, "")
        self.assertIn("Checked 3 JSON and YAML data files", result.stdout)

    def test_rejects_missing_and_empty_technology_localization(self):
        result = run_lint("bad_tech_locale_missing")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("missing English technology localization key modifier.tax_income", result.stderr)
        self.assertIn("tech.recordkeeping: expected nonempty English text", result.stderr)

    def test_rejects_duplicate_technology_localization_key(self):
        result = run_lint("bad_tech_locale")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("duplicate technology localization key tech.recordkeeping", result.stderr)

    def test_rejects_missing_duplicate_and_cyclic_technology_keys(self):
        result = run_lint("bad_tech_graph")
        self.assertEqual(result.returncode, 1, result)
        for message in (
            "duplicate technology key recordkeeping",
            "recordkeeping: missing prerequisite absent_technology",
            "cycle: professional_officers -> recordkeeping -> professional_officers",
        ):
            with self.subTest(message=message):
                self.assertIn(message, result.stderr)

    def test_rejects_invalid_technology_units_and_effects(self):
        result = run_lint("bad_tech_fields")
        self.assertEqual(result.returncode, 1, result)
        for field in (
            "$.technologies[0].key: expected an ASCII snake_case",
            "$.technologies[0].cost: expected an integer",
            "$.technologies[0].requires[1]: duplicate prerequisite missing",
            "$.technologies[0].pos.x: expected an integer",
            "$.technologies[0].pos.y: expected an integer",
            "$.technologies[0].effects[0].add_modifier.value: expected an integer",
        ):
            with self.subTest(field=field):
                self.assertIn(field, result.stderr)

    def test_rejects_two_files_for_one_technology_branch(self):
        result = run_lint("bad_tech_duplicate_branch")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("duplicate technology branch military", result.stderr)

    def test_rejects_timed_technology_modifier(self):
        result = run_lint("bad_tech_duration")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("technology effects must be permanent", result.stderr)

    def test_rejects_unknown_and_fractional_ai_technology_branch_weights(self):
        result = run_lint("bad_tech_profile")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("unknown technology branch", result.stderr)
        self.assertIn("must be a whole score 0..1000", result.stderr)

    def test_rejects_invalid_national_idea_fixture(self):
        result = run_lint("bad_idea")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("$.ideas[0].modifiers[0].value: expected an integer", result.stderr)
        self.assertIn("$.ideas[1].key: duplicate idea key", result.stderr)
        self.assertIn("$.ideas[1].modifiers[0].duration_days: unknown field", result.stderr)

    def test_rejects_idea_unlock_with_missing_technology(self):
        result = run_lint("bad_idea_reference")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("idea unlock references unknown technology absent_technology", result.stderr)

    def test_rejects_fractional_reform_modifier_value(self):
        result = run_lint("bad_reform")
        self.assertEqual(result.returncode, 1, result)
        self.assertIn("$.governments.monarchy[0].reforms[0].effects[0].value: expected an integer", result.stderr)


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
