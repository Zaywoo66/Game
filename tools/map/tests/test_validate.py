"""Regression tests for the map validator against the checked-in generated slice."""

from pathlib import Path
import shutil
import sys
from tempfile import TemporaryDirectory
import unittest

import numpy as np
from PIL import Image
from rasterio.features import rasterize
from rasterio.transform import from_bounds

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from map_common import BOUNDS, HEIGHT, INPUTS, OUTPUT, WIDTH, json_bytes, map_hash, read_json, read_yaml, yaml_bytes
from polity_evidence import build_evidence
from validate import validate_directory


class MapValidationTests(unittest.TestCase):
    def setUp(self):
        temporary = TemporaryDirectory(prefix="strategy-map-test-")
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        for name in (
            "provinces.json", "provinces.png", "heightmap.png", "terrain.png", "rivers.png",
            "areas.yaml", "regions.yaml", "names.en.json",
            "polities_1440.json",
        ):
            shutil.copyfile(OUTPUT / name, self.directory / name)
        self.metadata = read_json(self.directory / "provinces.json")

    def write_metadata(self, *, rehash=True):
        if rehash:
            self.metadata["map_hash"] = map_hash(
                self.metadata["provinces"], projection=self.metadata["projection"],
            )
        (self.directory / "provinces.json").write_bytes(json_bytes(self.metadata, pretty=True))

    def png(self, name):
        with Image.open(self.directory / name) as image:
            return np.array(image)

    def first_edge(self):
        for province in self.metadata["provinces"]:
            if province["neighbors"]:
                return province, province["neighbors"][0]
        self.fail("valid fixture should have a neighbor edge")

    def test_valid_slice(self):
        self.assertEqual(
            validate_directory(self.directory),
            {"land": 362, "sea": 66, "lake": 1},
        )

    def test_polity_evidence_covers_land_and_marks_uncertainty(self):
        evidence = read_json(self.directory / "polities_1440.json")
        assignments = evidence["assignments"]
        self.assertEqual(len(assignments), 362)
        self.assertEqual({entry["province_key"] for entry in assignments if entry["resolution"] == "unmatched"}, {
            "cork", "galway", "sligo", "lisburn", "killarney", "tralee", "omagh", "shannon", "donegal",
        })
        self.assertEqual(sum(entry["polity_name"] == "Holy Roman Empire Minor States" for entry in assignments), 67)
        self.assertTrue(all(entry["resolution"] == "coarse_group" for entry in assignments
                            if entry["polity_name"] == "Holy Roman Empire Minor States"))

    def test_polity_evidence_rebuilds_from_pinned_source(self):
        features = read_json(INPUTS / "historical_1440.geojson")["features"]
        history = rasterize(
            ((feature["geometry"], index) for index, feature in enumerate(features, 1)),
            out_shape=(HEIGHT, WIDTH), transform=from_bounds(*BOUNDS, WIDTH, HEIGHT), dtype=np.uint16,
        )
        rgb = self.png("provinces.png").astype(np.uint32)
        ids = (rgb[..., 0] << 16) | (rgb[..., 1] << 8) | rgb[..., 2]
        expected = build_evidence(ids, history, features, self.metadata["provinces"],
                                  self.metadata["map_version"], self.metadata["map_hash"])
        self.assertEqual(read_json(self.directory / "polities_1440.json"), expected)

    def test_polity_evidence_rejects_wrong_map_identity(self):
        path = self.directory / "polities_1440.json"
        evidence = read_json(path)
        evidence["map_hash"] = "0" * 64
        path.write_bytes(json_bytes(evidence, pretty=True))
        with self.assertRaisesRegex(ValueError, "map identity mismatch"):
            validate_directory(self.directory)

    def test_polity_evidence_rejects_wrong_source_digest(self):
        path = self.directory / "polities_1440.json"
        evidence = read_json(path)
        evidence["source"]["sha256"] = "0" * 64
        path.write_bytes(json_bytes(evidence, pretty=True))
        with self.assertRaisesRegex(ValueError, "source provenance mismatch"):
            validate_directory(self.directory)

    def test_polity_evidence_rejects_missing_and_overclaimed_matches(self):
        path = self.directory / "polities_1440.json"
        evidence = read_json(path)
        evidence["assignments"].pop()
        path.write_bytes(json_bytes(evidence, pretty=True))
        with self.assertRaisesRegex(ValueError, "cover every land province"):
            validate_directory(self.directory)
        evidence = read_json(OUTPUT / "polities_1440.json")
        coarse = next(entry for entry in evidence["assignments"] if entry["polity_name"] == "Holy Roman Empire Minor States")
        coarse["resolution"] = "polygon"
        path.write_bytes(json_bytes(evidence, pretty=True))
        with self.assertRaisesRegex(ValueError, "invalid source match or resolution"):
            validate_directory(self.directory)

    def test_unknown_area_is_rejected(self):
        self.metadata["provinces"][0]["area"] = "missing_area"
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "unknown area missing_area"):
            validate_directory(self.directory)

    def test_historical_area_requires_source(self):
        path = self.directory / "areas.yaml"
        hierarchy = read_yaml(path)
        historical = next(area for area in hierarchy["areas"] if area["kind"] == "historical_administration")
        historical["source"] = None
        path.write_bytes(yaml_bytes(hierarchy))
        with self.assertRaisesRegex(ValueError, "missing historical source"):
            validate_directory(self.directory)

    def test_wrong_projection_origin_is_rejected(self):
        self.metadata["projection"]["origin"][0] += 1
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "incorrect projection"):
            validate_directory(self.directory)

    def test_islands_do_not_gain_an_army_crossing(self):
        entries = self.metadata["provinces"]
        keys = {entry["id"]: entry["key"] for entry in entries}
        self.assertFalse(any(
            edge["edge"] == "strait" for entry in entries for edge in entry["neighbors"]
        ))
        land = {entry["key"]: entry for entry in entries if entry["kind"] == "land"}
        starts = {"london", "paris", "dublin", "palma", "cagliari", "ajaccio"}
        for start in starts:
            with self.subTest(start=start):
                seen, remaining = {start}, [start]
                while remaining:
                    current = remaining.pop()
                    for edge in land[current]["neighbors"]:
                        target = keys[edge["id"]]
                        if target in land and target not in seen:
                            seen.add(target)
                            remaining.append(target)
                self.assertEqual(seen & starts, {start})

    def test_reserved_zero_pixel_is_rejected(self):
        pixels = self.png("provinces.png")
        pixels[0, 0] = [0, 0, 0]
        Image.fromarray(pixels).save(self.directory / "provinces.png")
        with self.assertRaisesRegex(ValueError, "orphan pixels"):
            validate_directory(self.directory)

    def test_unknown_rgb24_province_is_rejected(self):
        pixels = self.png("provinces.png")
        pixels[0, 0] = [1, 0, 1]
        Image.fromarray(pixels).save(self.directory / "provinces.png")
        with self.assertRaisesRegex(ValueError, "raster IDs differ.*65537"):
            validate_directory(self.directory)

    def test_one_pixel_province_is_rejected(self):
        pixels = self.png("provinces.png")
        entry = self.metadata["provinces"][0]
        x, y = entry["centroid"]
        color = pixels[y, x].copy()
        mask = np.all(pixels == color, axis=2)
        mask[y, x] = False
        pixels[mask] = pixels[0, 0]
        Image.fromarray(pixels).save(self.directory / "provinces.png")
        with self.assertRaisesRegex(ValueError, "one-pixel province"):
            validate_directory(self.directory)

    def test_centroid_must_pick_its_province(self):
        self.metadata["provinces"][0]["centroid"] = self.metadata["provinces"][-1]["centroid"]
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "centroid does not select its own province"):
            validate_directory(self.directory)

    def test_undigested_metadata_change_is_rejected(self):
        self.metadata["provinces"][0]["development"] = 1
        self.write_metadata(rehash=False)
        with self.assertRaisesRegex(ValueError, "map_hash"):
            validate_directory(self.directory)

    def test_reverse_typed_edge_is_required(self):
        province, edge = self.first_edge()
        province["neighbors"].remove(edge)
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "missing reverse"):
            validate_directory(self.directory)

    def test_removing_both_directions_cannot_hide_a_pixel_boundary(self):
        province, edge = self.first_edge()
        province["neighbors"].remove(edge)
        target = next(p for p in self.metadata["provinces"] if p["id"] == edge["id"])
        target["neighbors"].remove({**edge, "id": province["id"]})
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "raster/override adjacency differs"):
            validate_directory(self.directory)

    def test_edge_cost_must_be_positive(self):
        _, edge = self.first_edge()
        edge["cost"] = 0
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "invalid edge cost"):
            validate_directory(self.directory)

    def test_adjacent_land_marches_take_three_to_ten_days(self):
        entries = {entry["id"]: entry for entry in self.metadata["provinces"]}
        traversable = [
            edge for entry in entries.values() if entry["kind"] == "land"
            for edge in entry["neighbors"] if entries[edge["id"]]["kind"] == "land"
        ]
        self.assertTrue(traversable)
        self.assertTrue(all(3 <= edge["cost"] <= 10 for edge in traversable))
        self.assertEqual({edge["cost"] for edge in traversable if edge["edge"] == "normal"}, {3})
        self.assertEqual({edge["cost"] for edge in traversable if edge["edge"] == "river"}, {5})

    def test_reverse_edge_cost_must_match(self):
        _, edge = self.first_edge()
        edge["cost"] += 1
        self.write_metadata()
        with self.assertRaisesRegex(ValueError, "edge cost mismatch"):
            validate_directory(self.directory)

    def test_heightmap_must_be_16_bit_grayscale(self):
        Image.fromarray((self.png("heightmap.png") >> 8).astype(np.uint8)).save(
            self.directory / "heightmap.png"
        )
        with self.assertRaisesRegex(ValueError, "incorrect PNG format"):
            validate_directory(self.directory)

    def test_rivers_must_be_binary(self):
        pixels = self.png("rivers.png")
        pixels[0, 0] = 1
        Image.fromarray(pixels).save(self.directory / "rivers.png")
        with self.assertRaisesRegex(ValueError, "binary 0/255"):
            validate_directory(self.directory)
