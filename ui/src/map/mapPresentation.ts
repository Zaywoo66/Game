import { MapAssets, Rgba } from './mapData';

export type MapMode = 'political' | 'terrain' | 'historical';

const CARTOGRAPHIC_PALETTE = Array.from({ length: 12 }, (_, index): Rgba => {
  const hue = index * 30;
  const saturation = 0.38;
  const lightness = 0.75;
  const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation;
  const segment = hue / 60;
  const secondary = chroma * (1 - Math.abs(segment % 2 - 1));
  const components = segment < 1 ? [chroma, secondary, 0]
    : segment < 2 ? [secondary, chroma, 0]
      : segment < 3 ? [0, chroma, secondary]
        : segment < 4 ? [0, secondary, chroma]
          : segment < 5 ? [secondary, 0, chroma] : [chroma, 0, secondary];
  const offset = lightness - chroma / 2;
  return [...components.map((component) => Math.round((component + offset) * 255)), 255] as Rgba;
});

function stableIndex(name: string): number {
  let hash = 2166136261;
  for (const character of name) hash = Math.imul(hash ^ character.codePointAt(0)!, 16777619);
  return hash >>> 0;
}

function paletteDistance(left: Rgba, right: Rgba): number {
  const red = left[0] - right[0];
  const green = left[1] - right[1];
  const blue = left[2] - right[2];
  return 0.30 * red * red + 0.59 * green * green + 0.11 * blue * blue;
}

function polityPalette(assets: MapAssets, names: Iterable<string>): Map<string, Rgba> {
  const polityNames = [...new Set(names)].sort();
  const neighbors = new Map(polityNames.map((name) => [name, new Set<string>()]));
  for (const attribution of assets.polityAttributions.values()) {
    const name = attribution.polity_name;
    if (!name) continue;
    for (const edge of assets.provinces.get(attribution.province_id)?.neighbors ?? []) {
      const adjacent = assets.polityAttributions.get(edge.id)?.polity_name;
      if (adjacent && adjacent !== name) {
        neighbors.get(name)?.add(adjacent);
        neighbors.get(adjacent)?.add(name);
      }
    }
  }

  const assigned = new Map<string, Rgba>();
  while (assigned.size < polityNames.length) {
    const remaining = polityNames.filter((name) => !assigned.has(name));
    remaining.sort((left, right) => {
      const leftColors = new Set([...neighbors.get(left)!].map((name) => assigned.get(name)).filter(Boolean));
      const rightColors = new Set([...neighbors.get(right)!].map((name) => assigned.get(name)).filter(Boolean));
      return rightColors.size - leftColors.size
        || neighbors.get(right)!.size - neighbors.get(left)!.size
        || left.localeCompare(right);
    });
    const selected = remaining[0];
    const adjacentColors = [...neighbors.get(selected)!]
      .map((name) => assigned.get(name)).filter((color): color is Rgba => color !== undefined);
    const offset = stableIndex(selected) % CARTOGRAPHIC_PALETTE.length;
    const paletteIndices = CARTOGRAPHIC_PALETTE.map((_, index) => index)
      .sort((left, right) => ((left - offset + 12) % 12) - ((right - offset + 12) % 12));
    let bestIndex = paletteIndices[0];
    let bestDistance = -1;
    for (const index of paletteIndices) {
      const color = CARTOGRAPHIC_PALETTE[index];
      const minimumDistance = adjacentColors.length
        ? Math.min(...adjacentColors.map((neighbor) => paletteDistance(color, neighbor)))
        : Number.POSITIVE_INFINITY;
      if (minimumDistance > bestDistance) {
        bestDistance = minimumDistance;
        bestIndex = index;
      }
    }
    assigned.set(selected, CARTOGRAPHIC_PALETTE[bestIndex]);
  }
  return assigned;
}

export function historicalPresentation(assets: MapAssets) {
  const colors = new Map<number, Rgba>();
  const precision = new Map<number, number>();
  const groups = new Map<string, number[]>();
  const palette = polityPalette(assets, [...assets.polityAttributions.values()]
    .flatMap(({ polity_name }) => polity_name ? [polity_name] : []));
  for (const attribution of assets.polityAttributions.values()) {
    const { province_id: id, polity_name: name, resolution } = attribution;
    colors.set(id, name === null ? [113, 120, 116, 255] : palette.get(name)!);
    precision.set(id, resolution === 'polygon' ? 1 : resolution === 'coarse_group' ? 2 : 3);
    if (name) {
      const current = groups.get(name) ?? [];
      current.push(id);
      groups.set(name, current);
    }
  }
  const labels = [...groups].map(([name, ids]) => {
    const remaining = new Set(ids);
    let largest: number[] = [];
    let largestPixels = 0;
    while (remaining.size) {
      const start = remaining.values().next().value as number;
      const component: number[] = [];
      const stack = [start];
      remaining.delete(start);
      while (stack.length) {
        const id = stack.pop()!;
        component.push(id);
        for (const neighbor of assets.provinces.get(id)?.neighbors ?? []) {
          if (remaining.delete(neighbor.id)) stack.push(neighbor.id);
        }
      }
      const pixels = component.reduce((sum, id) => sum + (assets.polityAttributions.get(id)?.province_pixels ?? 0), 0);
      if (pixels > largestPixels) { largest = component; largestPixels = pixels; }
    }
    const mean = largest.reduce((sum, id) => {
      const [x, y] = assets.provinces.get(id)!.centroid;
      const weight = assets.polityAttributions.get(id)!.province_pixels;
      return [sum[0] + x * weight, sum[1] + y * weight];
    }, [0, 0]);
    const center = [mean[0] / largestPixels, mean[1] / largestPixels];
    const id = largest.reduce((best, candidate) => {
      const [x, y] = assets.provinces.get(candidate)!.centroid;
      const [bx, by] = assets.provinces.get(best)!.centroid;
      return (x - center[0]) ** 2 + (y - center[1]) ** 2 < (bx - center[0]) ** 2 + (by - center[1]) ** 2
        ? candidate : best;
    }, largest[0]);
    return { id, text: name, weight: largestPixels };
  }).sort((left, right) => right.weight - left.weight);
  return { colors, precision, labels };
}
