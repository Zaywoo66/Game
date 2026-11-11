import { MapAssets, Rgba } from './mapData';

export type MapMode = 'political' | 'terrain' | 'historical';

function polityColor(name: string): Rgba {
  let hash = 2166136261;
  for (const character of name) hash = Math.imul(hash ^ character.codePointAt(0)!, 16777619);
  const hue = (hash >>> 0) % 360;
  const saturation = 0.42;
  const lightness = 0.55;
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
}

export function historicalPresentation(assets: MapAssets) {
  const colors = new Map<number, Rgba>();
  const precision = new Map<number, number>();
  const groups = new Map<string, number[]>();
  for (const attribution of assets.polityAttributions.values()) {
    const { province_id: id, polity_name: name, resolution } = attribution;
    colors.set(id, name === null ? [113, 120, 116, 255] : polityColor(name));
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
