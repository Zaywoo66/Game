import { MapAssets, Rgba } from './mapData';

export type MapMode = 'political' | 'terrain' | 'historical';

const CARTOGRAPHIC_PALETTE = Array.from({ length: 108 }, (_, index): Rgba => {
  const hue = (index % 36) * 10;
  const tier = Math.floor(index / 36);
  const saturation = [0.43, 0.50, 0.46][tier];
  const lightness = [0.70, 0.65, 0.75][tier];
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

function hslColor(hue: number, saturation: number, lightness: number): Rgba {
  const chroma = (1 - Math.abs(2 * lightness - 1)) * saturation;
  const segment = ((hue % 360) + 360) % 360 / 60;
  const secondary = chroma * (1 - Math.abs(segment % 2 - 1));
  const components = segment < 1 ? [chroma, secondary, 0]
    : segment < 2 ? [secondary, chroma, 0]
      : segment < 3 ? [0, chroma, secondary]
        : segment < 4 ? [0, secondary, chroma]
          : segment < 5 ? [secondary, 0, chroma] : [chroma, 0, secondary];
  const offset = lightness - chroma / 2;
  return [...components.map((component) => Math.round((component + offset) * 255)), 255] as Rgba;
}

function paletteDistance(left: Rgba, right: Rgba): number {
  const linear = (value: number) => {
    const channel = value / 255;
    return channel <= 0.04045 ? channel / 12.92 : ((channel + 0.055) / 1.055) ** 2.4;
  };
  const [red, green, blue] = [left[0], left[1], left[2]].map(linear);
  const l = Math.cbrt(0.4122214708 * red + 0.5363325363 * green + 0.0514459929 * blue);
  const m = Math.cbrt(0.2119034982 * red + 0.6806995451 * green + 0.1073969566 * blue);
  const s = Math.cbrt(0.0883024619 * red + 0.2817188376 * green + 0.6299787005 * blue);
  const labL = 0.2104542553 * l + 0.7936177850 * m - 0.0040720468 * s;
  const labA = 1.9779984951 * l - 2.4285922050 * m + 0.4505937099 * s;
  const labB = 0.0259040371 * l + 0.7827717662 * m - 0.8086757660 * s;
  const [otherRed, otherGreen, otherBlue] = [right[0], right[1], right[2]].map(linear);
  const otherLms = [
    Math.cbrt(0.4122214708 * otherRed + 0.5363325363 * otherGreen + 0.0514459929 * otherBlue),
    Math.cbrt(0.2119034982 * otherRed + 0.6806995451 * otherGreen + 0.1073969566 * otherBlue),
    Math.cbrt(0.0883024619 * otherRed + 0.2817188376 * otherGreen + 0.6299787005 * otherBlue),
  ];
  const otherL = 0.2104542553 * otherLms[0] + 0.7936177850 * otherLms[1] - 0.0040720468 * otherLms[2];
  const otherA = 1.9779984951 * otherLms[0] - 2.4285922050 * otherLms[1] + 0.4505937099 * otherLms[2];
  const otherB = 0.0259040371 * otherLms[0] + 0.7827717662 * otherLms[1] - 0.8086757660 * otherLms[2];
  return (labL - otherL) ** 2 + (labA - otherA) ** 2 + (labB - otherB) ** 2;
}

function polityPalette(assets: MapAssets, names: Iterable<string>): Map<string, Rgba> {
  const polityNames = [...new Set(names)].sort();
  const neighbors = new Map(polityNames.map((name) => [name, new Set<string>()]));
  for (const attribution of assets.polityAttributions.values()) {
    if (attribution.resolution !== 'polygon') continue;
    const name = attribution.polity_name;
    if (!name) continue;
    for (const edge of assets.provinces.get(attribution.province_id)?.neighbors ?? []) {
      const adjacentAttribution = assets.polityAttributions.get(edge.id);
      const adjacent = adjacentAttribution?.resolution === 'polygon'
        ? adjacentAttribution.polity_name : null;
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
      .sort((left, right) => ((left - offset + CARTOGRAPHIC_PALETTE.length) % CARTOGRAPHIC_PALETTE.length)
        - ((right - offset + CARTOGRAPHIC_PALETTE.length) % CARTOGRAPHIC_PALETTE.length));
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
  const kalmarRegions = new Map<string, number[]>();
  const palette = polityPalette(assets, [...assets.polityAttributions.values()]
    .flatMap(({ polity_name, resolution }) => resolution === 'polygon' && polity_name ? [polity_name] : []));
  for (const attribution of assets.polityAttributions.values()) {
    const { province_id: id, polity_name: name, resolution } = attribution;
    const isNamedPolygon = resolution === 'polygon' && name !== null;
    precision.set(id, resolution === 'polygon' ? 1 : resolution === 'coarse_group' ? 2 : 3);
    if (isNamedPolygon && name) {
      colors.set(id, palette.get(name)!);
      const current = groups.get(name) ?? [];
      current.push(id);
      groups.set(name, current);
    }
  }

  // Keep the reference map fully colored while making unresolved territory
  // visually distinct from attributed polities and authoritative ownership.
  // HRE placeholder cells use independent muted hues, never one imperial fill.
  // The coarse Kalmar source group is visually split by geographic subregions
  // to respect the owner's note that its members remain separate kingdoms.
  // These tints are cosmetic only; they never become country ownership.
  const unresolved = [...assets.polityAttributions.values()]
    .filter(({ resolution, polity_name }) => resolution !== 'polygon' || polity_name === null)
    .sort((left, right) => left.province_id - right.province_id);
  for (const { province_id: id, polity_name: sourceGroup } of unresolved) {
    const key = assets.provinces.get(id)?.key ?? String(id);
    const seed = stableIndex(key);
    const group = sourceGroup?.toLowerCase() ?? '';
    let centerHue = 34;
    let spread = 30;
    let saturation = 0.22 + ((seed >>> 8) % 15) / 100;
    let lightness = 0.68 + ((seed >>> 16) % 9) / 100;
    if (group.includes('holy roman')) {
      // The HRE was decentralized: keep its placeholder cells distinct and
      // restrained instead of presenting a unified imperial color.
      centerHue = seed % 360;
      spread = 0;
      saturation = 0.35 + ((seed >>> 8) % 12) / 100;
      lightness = 0.57 + ((seed >>> 16) % 10) / 100;
    } else if (group.includes('kalmar')) {
      const province = assets.provinces.get(id);
      const scale = assets.projection.pixels_per_degree;
      const [originX, originY] = assets.projection.origin;
      const longitude = province
        ? province.centroid[0] / scale + originX / scale - 180 : 12;
      const latitude = province
        ? 90 - (province.centroid[1] / scale + originY / scale) : 60;
      const family = longitude < -10 ? 'iceland'
        : latitude < 57.8 && longitude < 16 ? 'denmark'
          : longitude < 10.3 && latitude > 57.5 ? 'norway' : 'sweden';
      const name = family === 'iceland' ? 'Iceland'
        : `Kingdom of ${family[0].toUpperCase()}${family.slice(1)}`;
      const current = kalmarRegions.get(name) ?? [];
      current.push(id);
      kalmarRegions.set(name, current);
      // The reference has separate Scandinavian realms. These muted
      // geographic tints communicate that split without assigning ownership.
      centerHue = family === 'denmark' ? 8 : family === 'norway' ? 207
        : family === 'iceland' ? 188 : 45;
      spread = 0;
      saturation = 0.42;
      lightness = 0.56;
    }
    const hue = centerHue + (spread ? (seed % (spread * 2 + 1)) - spread : 0);
    colors.set(id, hslColor(hue, saturation, lightness));
  }

  const labelForGroup = (name: string, ids: number[]) => {
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
  };
  const labels = [
    ...[...groups].map(([name, ids]) => labelForGroup(name, ids)),
    ...[...kalmarRegions].map(([name, ids]) => labelForGroup(name, ids)),
  ].sort((left, right) => right.weight - left.weight);
  return { colors, precision, labels };
}
