"""Pixel geometry and graph evidence shared by the generator and validator."""

from collections import Counter
import math

import numpy as np

from map_common import BOUNDS

PAIR_SHIFT = 24
PAIR_MASK = (1 << PAIR_SHIFT) - 1


def pixel_for_point(point, shape):
    lon, lat = point
    west, south, east, north = BOUNDS
    height, width = shape
    x = math.floor((lon - west) / (east - west) * width)
    y = math.floor((north - lat) / (north - south) * height)
    if not (0 <= x < width and 0 <= y < height):
        raise ValueError(f"point {point} is outside the map raster")
    return y, x


def snap_point(point, mask, radius=8):
    y, x = pixel_for_point(point, mask.shape)
    height, width = mask.shape
    top, bottom = max(0, y - radius), min(height, y + radius + 1)
    left, right = max(0, x - radius), min(width, x + radius + 1)
    candidates = np.argwhere(mask[top:bottom, left:right])
    if len(candidates) == 0:
        raise ValueError(f"point {point} has no matching raster pixel within {radius} px")
    candidates += (top, left)
    distances = ((candidates - (y, x)) ** 2).sum(axis=1)
    nearest = int(distances.argmin())
    if distances[nearest] > radius**2:
        raise ValueError(f"point {point} has no matching raster pixel within {radius} px")
    return tuple(map(int, candidates[nearest]))


def paired_slices(shape, dy, dx):
    height, width = shape
    return (
        (slice(0, height - dy), slice(max(-dx, 0), width - max(dx, 0))),
        (slice(dy, height), slice(max(dx, 0), width - max(-dx, 0))),
    )


def raster_contacts(ids, river):
    """Counts cardinal pixel contacts, with separate river evidence."""
    contacts, river_contacts = Counter(), Counter()
    for dy, dx in ((0, 1), (1, 0)):
        a, b = paired_slices(ids.shape, dy, dx)
        mask = ids[a] != ids[b]
        low = np.minimum(ids[a][mask], ids[b][mask]).astype(np.uint64)
        high = np.maximum(ids[a][mask], ids[b][mask]).astype(np.uint64)
        codes = (low << PAIR_SHIFT) | high
        for counter, samples in (
            (contacts, codes),
            (river_contacts, codes[(river[a] | river[b])[mask]]),
        ):
            unique, counts = np.unique(samples, return_counts=True)
            for code, count in zip(unique, counts, strict=True):
                encoded = int(code)
                counter[(encoded >> PAIR_SHIFT, encoded & PAIR_MASK)] += int(count)
    return contacts, river_contacts


def regular_edge(pair, contacts, river_contacts, kinds):
    river_pixels = river_contacts[pair]
    if (
        kinds[pair[0]] == kinds[pair[1]] == "land"
        and river_pixels >= 3
        and 5 * river_pixels >= contacts[pair]
    ):
        return "river"
    return "normal"


def strait_pairs(ids, land, sea, overrides):
    """Resolve named crossings to distinct land provinces with water between them."""
    pairs = set()
    for entry in overrides:
        first, second = (snap_point(point, land) for point in entry["endpoints"])
        pair = tuple(sorted((int(ids[first]), int(ids[second]))))
        if pair[0] == pair[1] or pair in pairs:
            raise ValueError(f"{entry['name']}: duplicate or same-province strait {pair}")
        steps = 1 + 2 * max(abs(first[0] - second[0]), abs(first[1] - second[1]))
        y, x = (
            np.rint(np.linspace(start, end, steps)).astype(np.intp)
            for start, end in zip(first, second, strict=True)
        )
        if np.count_nonzero(sea[y, x]) < 4:
            raise ValueError(f"{entry['name']}: crossing must span open sea")
        pairs.add(pair)
    return pairs
