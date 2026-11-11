"""Generate the offline Western Europe M2 map from pinned source subsets."""

import argparse
from collections import Counter
from pathlib import Path
from tempfile import TemporaryDirectory

import numpy as np
from PIL import Image
from rasterio.features import rasterize
from rasterio.transform import from_bounds
from scipy import ndimage
from scipy.sparse import csr_matrix
from scipy.sparse.csgraph import dijkstra

from map_common import (
    BOUNDS, EDGE_ORDER, ELEVATION_BIAS, HEIGHT, INPUTS, OUTPUT, PROJECTION, REGION, SCHEMA_VERSION, TERRAINS,
    WIDTH, json_bytes, map_hash, read_json, read_yaml, yaml_bytes,
)
from validate import validate_directory
from historical_areas import classify_seeds, slug, update_ownership
from topology import paired_slices, raster_contacts, regular_edge, snap_point, strait_pairs

SHAPE = (HEIGHT, WIDTH)
TRANSFORM = from_bounds(*BOUNDS, WIDTH, HEIGHT)
Y, X = np.ogrid[:HEIGHT, :WIDTH]


def features(layer):
    return read_json(INPUTS / (layer + ".geojson"))["features"]


def mask(layer, *, all_touched=False):
    return rasterize(
        ((feature["geometry"], 1) for feature in features(layer)),
        out_shape=SHAPE, transform=TRANSFORM, dtype=np.uint8, all_touched=all_touched,
    ).astype(bool)


def raster_elevation():
    west, south, east, north = BOUNDS
    with np.load(INPUTS / "elevation.npz") as source:
        # Convert map pixel centers to fractional source sample coordinates.
        y = np.interp(
            north - (np.arange(HEIGHT) + 0.5) * (north - south) / HEIGHT,
            source["latitude"], np.arange(len(source["latitude"])),
        )
        x = np.interp(
            west + (np.arange(WIDTH) + 0.5) * (east - west) / WIDTH,
            source["longitude"], np.arange(len(source["longitude"])),
        )
        samples = ndimage.map_coordinates(
            source["altitude"].astype(np.float64),
            np.broadcast_arrays(y[:, None], x[None, :]), order=1, mode="nearest",
        )
    return np.rint(samples).astype(np.int16)


def water_centers(sea, columns, rows, minimum_coverage_percent):
    for row in range(rows):
        top, bottom = HEIGHT * row // rows, HEIGHT * (row + 1) // rows
        for column in range(columns):
            left, right = WIDTH * column // columns, WIDTH * (column + 1) // columns
            cell = sea[top:bottom, left:right]
            if 100 * np.count_nonzero(cell) < minimum_coverage_percent * cell.size:
                continue
            pixels = np.argwhere(cell) + (top, left)
            center = ((top + bottom - 1) / 2, (left + right - 1) / 2)
            distance = ((pixels - center) ** 2).sum(axis=1)
            yield tuple(map(int, pixels[distance.argmin()]))


def seeds_from_config(config, masks):
    seeds, names, skipped = {}, {}, []
    required = set(config["required_cities"]) | set(config["port_provinces"])
    place_keys = config["place_keys"]
    places = features("populated_places")
    places.sort(key=lambda place: (
        place_keys.get(place["properties"]["name"], slug(place["properties"]["name"])) not in required,
        place["properties"]["rank"], -place["properties"]["population"],
        place["properties"]["name"], place["geometry"]["coordinates"],
    ))
    occupied = set()
    for place in places:
        name = place["properties"]["name"]
        key = place_keys.get(name, slug(name))
        try:
            y, x = snap_point(place["geometry"]["coordinates"], masks["land"], radius=5)
        except ValueError:
            skipped.append(name)
            continue
        near = any(
            (y - dy, x - dx) in occupied
            for dy in range(-2, 3) for dx in range(-2, 3) if dx * dx + dy * dy < 9
        )
        if near:
            skipped.append(name)
            continue
        if key in names:
            raise ValueError(f"duplicate province key {key}")
        occupied.add((y, x))
        seeds[len(seeds) + 1] = (key, "land", (y, x))
        names[key] = name
    if required - names.keys():
        raise ValueError(f"required land seeds are missing: {sorted(required - names.keys())}")

    sea_config = config["sea_grid"]
    for index, point in enumerate(water_centers(masks["sea"], **sea_config), start=1):
        key = f"sea_zone_{index:02}"
        seeds[len(seeds) + 1] = (key, "sea", point)
        names[key] = f"Sea Zone {index}"
    for key, position in config["lake"].items():
        seeds[len(seeds) + 1] = (key, "lake", snap_point(position, masks["lake"]))
        names[key] = key.replace("_", " ").title()
    if not 400 <= len(seeds) <= 600:
        raise ValueError(f"M5b map must have 400 to 600 province seeds, found {len(seeds)}")
    return seeds, names, skipped


def nearest_pixels(seeds):
    best = np.full(SHAPE, np.iinfo(np.int32).max, dtype=np.int32)
    labels = np.zeros(SHAPE, dtype=np.uint32)
    for province, (_, _, (y, x)) in seeds.items():
        distance = (Y - y) ** 2 + (X - x) ** 2
        better = distance < best
        best[better] = distance[better]
        labels[better] = province
    return labels


def land_cells(land, rivers, polities, seeds):
    """Raster Voronoi with shoreline barriers and costs at rivers and 1440 borders."""
    cells = np.arange(HEIGHT * WIDTH, dtype=np.int32).reshape(SHAPE)
    n_pixels = cells.size
    rows, cols, weights = [], [], []
    # Bias ties toward the lower seed ID. Every step costs a multiple of
    # stride, so the bias can never make a geometrically longer route win.
    stride = max(seeds) + 1
    for dy, dx, base in ((0, 1, 10), (1, 0, 10), (1, 1, 14), (1, -1, 14)):
        a, b = paired_slices(SHAPE, dy, dx)
        valid = land[a] & land[b]
        if dy and dx:
            valid &= (land[a[0], b[1]] | land[b[0], a[1]])
        u, v = cells[a][valid], cells[b][valid]
        river_crossing = (rivers[a] != rivers[b])[valid]
        border = ((polities[a] != polities[b]) & (polities[a] > 0) & (polities[b] > 0))[valid]
        cost = stride * (base + 24 * river_crossing + 160 * border)
        rows.extend((u, v))
        cols.extend((v, u))
        weights.extend((cost, cost))

    # Extra source nodes attach the tie bias once at the start of each path.
    source_nodes = np.arange(n_pixels, n_pixels + len(seeds), dtype=np.int32)
    rows.append(source_nodes)
    cols.append(np.array([cells[seed] for _, _, seed in seeds.values()], dtype=np.int32))
    weights.append(np.array(list(seeds), dtype=np.int32))
    adjacency = csr_matrix(
        (np.concatenate(weights), (np.concatenate(rows), np.concatenate(cols))),
        shape=(n_pixels + len(seeds), n_pixels + len(seeds)),
    )
    _, _, closest = dijkstra(
        adjacency, directed=True, indices=source_nodes, min_only=True,
        return_predecessors=True,
    )
    labels = np.zeros(SHAPE, dtype=np.uint32)
    owners = closest[:n_pixels].reshape(SHAPE)
    reachable = land & (owners >= n_pixels)
    labels[reachable] = np.array(list(seeds), dtype=np.uint32)[owners[reachable] - n_pixels]
    # Land fragments without seeds (small offshore islands) stay land and
    # attach as whole components to the geographically nearest land seed.
    islands, count = ndimage.label(land & (labels == 0), structure=np.ones((3, 3)))
    for island, bounds in enumerate(ndimage.find_objects(islands), start=1):
        local = islands[bounds] == island
        iy, ix = np.nonzero(local)
        y, x = iy.mean() + bounds[0].start, ix.mean() + bounds[1].start
        nearest = min(
            seeds, key=lambda province: (
                (seeds[province][2][0] - y) ** 2 + (seeds[province][2][1] - x) ** 2,
                province,
            )
        )
        labels[bounds][local] = nearest
    return labels, count


def province_records(ids, terrain, rivers, masks, seeds, config, assignments):
    keys = {key: province for province, (key, _, _) in seeds.items()}
    kinds = {province: kind for province, (_, kind, _) in seeds.items()}
    contacts, river_contacts = raster_contacts(ids, rivers)
    edges = {province: set() for province in seeds}
    ports = {province: [] for province in seeds}

    def link(a, b, kind):
        edges[a].add((b, kind))
        edges[b].add((a, kind))

    for pair in contacts:
        link(*pair, regular_edge(pair, contacts, river_contacts, kinds))
    for pair in strait_pairs(ids, masks["land"], masks["sea"], config["straits"]):
        if pair in contacts:
            raise ValueError(f"strait endpoints already share a raster boundary: {pair}")
        link(*pair, "strait")
    for key in config["port_provinces"]:
        land = keys[key]
        if kinds[land] != "land":
            raise ValueError(f"{key}: a port override must name a land province")
        sea_neighbors = [
            (target, contacts[tuple(sorted((land, target)))])
            for target, kind in edges[land]
            if kinds[target] == "sea" and kind == "normal"
        ]
        if not sea_neighbors:
            raise ValueError(f"{key}: a port override must touch sea")
        sea, _ = min(sea_neighbors, key=lambda item: (-item[1], item[0]))
        ports[land].append(sea)
        link(land, sea, "port")

    provinces = []
    for province, (key, kind, _) in seeds.items():
        py, px = np.nonzero(ids == province)
        # Snap the pixel centroid inside the province, including concave cells.
        nearest = np.argmin((py - py.mean()) ** 2 + (px - px.mean()) ** 2)
        dominant = np.bincount(terrain[py, px]).argmax()
        provinces.append({
            "id": province,
            "key": key,
            "kind": kind,
            "terrain": TERRAINS[dominant],
            "centroid": [int(px[nearest]), int(py[nearest])],
            "neighbors": [
                {"id": other, "edge": edge, "cost": config["movement_costs"][edge]}
                for other, edge in sorted(edges[province], key=lambda item: (item[0], EDGE_ORDER.index(item[1])))
            ],
            "ports": sorted(ports[province]),
            "development": 0,
            "name_key": f"province_{key}",
            "area": assignments[key],
        })
    return provinces


def generate(output):
    config = read_yaml(INPUTS / "overrides.yaml")
    physical_land = mask("land")
    lake = mask("lakes") & physical_land
    masks = {"land": physical_land & ~lake, "sea": ~physical_land, "lake": lake}
    rivers = mask("rivers", all_touched=True) & masks["land"]
    historical_features = features("historical_1440")
    history = rasterize(
        ((feature["geometry"], i) for i, feature in enumerate(historical_features, start=1)),
        out_shape=SHAPE, transform=TRANSFORM, dtype=np.uint16,
    )
    seeds, names, skipped = seeds_from_config(config, masks)
    land_seeds = {province: info for province, info in seeds.items() if info[1] == "land"}
    ids, detached = land_cells(masks["land"], rivers, history, land_seeds)
    for kind in ("sea", "lake"):
        water_seeds = {province: info for province, info in seeds.items() if info[1] == kind}
        closest = nearest_pixels(water_seeds)
        ids[masks[kind]] = closest[masks[kind]]

    elevation = raster_elevation()
    terrain = np.zeros(SHAPE, dtype=np.uint8)
    terrain[masks["land"]] = 1
    terrain[masks["land"] & (elevation >= 350)] = 2
    terrain[masks["land"] & (elevation >= 1400)] = 3
    terrain[masks["lake"]] = 4
    assignments, areas, regions, ownership = classify_seeds(seeds, ids, history, historical_features, config)
    provinces = province_records(ids, terrain, rivers, masks, seeds, config, assignments)
    metadata = {
        "schema_version": SCHEMA_VERSION,
        "map_version": config["map_version"],
        "map_hash": map_hash(provinces, projection=PROJECTION),
        "projection": PROJECTION,
        "provinces": provinces,
    }
    rgb = np.stack((ids >> 16, ids >> 8, ids), axis=-1).astype(np.uint8)
    output.mkdir(parents=True, exist_ok=True)
    Image.fromarray(rgb).save(output / "provinces.png", compress_level=9)
    Image.fromarray((elevation.astype(np.int32) + ELEVATION_BIAS).astype("<u2")).save(
        output / "heightmap.png", compress_level=9,
    )
    Image.fromarray(terrain).save(output / "terrain.png", compress_level=9)
    Image.fromarray(rivers.astype(np.uint8) * 255).save(output / "rivers.png", compress_level=9)
    (output / "provinces.json").write_bytes(json_bytes(metadata, pretty=True))
    (output / "areas.yaml").write_bytes(yaml_bytes(areas))
    (output / "regions.yaml").write_bytes(yaml_bytes(regions))
    labels = {f"province_{key}": value for key, value in names.items()}
    labels.update({entry["name_key"]: entry["name"] for entry in areas["areas"]})
    labels.update({entry["name_key"]: entry["name"] for entry in regions["regions"]})
    (output / "names.en.json").write_bytes(json_bytes(labels, pretty=True))
    return Counter(entry["kind"] for entry in provinces), detached, metadata["map_hash"], ownership, skipped


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--region", choices=(REGION,), required=True)
    parser.add_argument("--update-scenario", action="store_true", help="Rebuild 1440 land ownership from the historical input")
    args = parser.parse_args()
    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory(prefix=f".{args.region}-", dir=OUTPUT.parent) as temporary:
        stage = Path(temporary)
        kinds, islands, digest, ownership, skipped = generate(stage)
        validate_directory(stage)
        OUTPUT.mkdir(parents=True, exist_ok=True)
        for path in sorted(stage.iterdir()):
            path.replace(OUTPUT / path.name)
    if args.update_scenario:
        update_ownership(ownership)
    print(f"Skipped settlement seeds: {', '.join(skipped)}")
    print(f"{args.region}: {sum(kinds.values())} provinces {dict(kinds)}, {WIDTH}x{HEIGHT}, SHA-256 {digest}")
    print(f"{islands} unseeded land fragments attached to their nearest land seed")


if __name__ == "__main__":
    main()
