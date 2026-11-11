"""Regression tests for the map validator against the checked-in generated slice."""

from pathlib import Path
import shutil
import sys
from tempfile import TemporaryDirectory
import unittest

import numpy as np
from PIL import Image

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

from map_common import OUTPUT, json_bytes, map_hash, read_json
from validate import validate_directory


class MapValidationTests(unittest.TestCase):
    def setUp(self):
        temporary = TemporaryDirectory(prefix="strategy-map-test-")
        self.addCleanup(temporary.cleanup)
        self.directory = Path(temporary.name)
        for name in ("provinces.json", "provinces.png", "heightmap.png", "terrain.png", "rivers.png"):
            shutil.copyfile(OUTPUT / name, self.directory / name)
        self.metadata = read_json(self.directory / "provinces.json")

    def write_metadata(self, *, rehash=True):
        if rehash:
            self.metadata["map_hash"] = map_hash(self.metadata["provinces"])
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
            {"land": 42, "sea": 6, "lake": 1},
        )

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
