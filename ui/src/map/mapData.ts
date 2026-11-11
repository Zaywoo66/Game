export type Rgba = [number, number, number, number];

export interface Raster {
  width: number;
  height: number;
  data: Uint8ClampedArray;
}

export interface Heightmap {
  width: number;
  height: number;
  samples: Uint16Array;
}

export function decodeProvinceId(red: number, green: number, blue: number): number {
  return (red << 16) | (green << 8) | blue;
}

/** Coordinates are normalized from the northwest corner, with v increasing south. */
export function pickProvinceId(
  rgba: Uint8ClampedArray | Uint8Array,
  width: number,
  height: number,
  u: number,
  v: number,
): number {
  if (!Number.isFinite(u) || !Number.isFinite(v) || u < 0 || u > 1 || v < 0 || v > 1) {
    return 0;
  }
  const x = Math.min(width - 1, Math.floor(u * width));
  const y = Math.min(height - 1, Math.floor(v * height));
  const i = (y * width + x) * 4;
  return decodeProvinceId(rgba[i], rgba[i + 1], rgba[i + 2]);
}

/** Changes exactly one RGBA record in the owner lookup. Returns whether it changed. */
export function updateOwnerLookup(bytes: Uint8Array, id: number, rgba: Rgba): boolean {
  if (!Number.isInteger(id) || id <= 0 || id >= bytes.length / 4) {
    throw new RangeError('Province ID outside owner lookup');
  }
  if (rgba.length !== 4 || rgba.some((value) => !Number.isInteger(value) || value < 0 || value > 255)) {
    throw new RangeError('Owner colors must contain four byte values');
  }
  const offset = id * 4;
  if (rgba.every((value, channel) => bytes[offset + channel] === value)) {
    return false;
  }
  bytes.set(rgba, offset);
  return true;
}

const pngSignature = [137, 80, 78, 71, 13, 10, 26, 10];

function paeth(left: number, above: number, upperLeft: number): number {
  const estimate = left + above - upperLeft;
  const fromLeft = Math.abs(estimate - left);
  const fromAbove = Math.abs(estimate - above);
  const fromUpperLeft = Math.abs(estimate - upperLeft);
  if (fromLeft <= fromAbove && fromLeft <= fromUpperLeft) return left;
  return fromAbove <= fromUpperLeft ? above : upperLeft;
}

/** Reads lossless samples from the noninterlaced grayscale PNG emitted by the map tool. */
export async function decodeHeightmap16(buffer: ArrayBuffer): Promise<Heightmap> {
  const file = new Uint8Array(buffer);
  const view = new DataView(buffer);
  if (!pngSignature.every((value, i) => file[i] === value)) {
    throw new Error('Heightmap does not have a PNG signature');
  }

  let width = 0;
  let height = 0;
  let finished = false;
  const compressed: ArrayBuffer[] = [];
  for (let chunk = 8; chunk + 12 <= file.length;) {
    const length = view.getUint32(chunk);
    const type = String.fromCharCode(...file.subarray(chunk + 4, chunk + 8));
    const start = chunk + 8;
    const end = start + length;
    if (end + 4 > file.length) throw new Error('Truncated heightmap PNG chunk');

    if (type === 'IHDR') {
      if (chunk !== 8 || length !== 13 || file[start + 8] !== 16
        || file[start + 9] !== 0 || file[start + 10] !== 0
        || file[start + 11] !== 0 || file[start + 12] !== 0) {
        throw new Error('Heightmap must be a noninterlaced 16-bit grayscale PNG');
      }
      width = view.getUint32(start);
      height = view.getUint32(start + 4);
    } else if (type === 'IDAT') {
      compressed.push(buffer.slice(start, end));
    } else if (type === 'IEND') {
      finished = true;
      break;
    }
    chunk = end + 4;
  }

  if (width === 0 || height === 0 || width * height > 4096 * 2048 || !finished || compressed.length === 0) {
    throw new Error('Heightmap PNG has invalid dimensions or missing image data');
  }

  const stream = new Blob(compressed).stream().pipeThrough(new DecompressionStream('deflate'));
  const rows = new Uint8Array(await new Response(stream).arrayBuffer());
  const stride = width * 2;
  if (rows.length !== (stride + 1) * height) throw new Error('Invalid heightmap scanline size');

  const samples = new Uint16Array(width * height);
  let previous = new Uint8Array(stride);
  let row = new Uint8Array(stride);
  for (let y = 0; y < height; y++) {
    const start = y * (stride + 1);
    const filter = rows[start];
    if (filter > 4) throw new Error('Invalid heightmap PNG filter');

    for (let x = 0; x < stride; x++) {
      const left = x < 2 ? 0 : row[x - 2];
      const above = previous[x];
      const upperLeft = x < 2 ? 0 : previous[x - 2];
      let predictor = 0;
      if (filter === 1) predictor = left;
      if (filter === 2) predictor = above;
      if (filter === 3) predictor = (left + above) >> 1;
      if (filter === 4) predictor = paeth(left, above, upperLeft);
      row[x] = rows[start + x + 1] + predictor;
    }
    for (let x = 0; x < width; x++) {
      samples[y * width + x] = (row[x * 2] << 8) | row[x * 2 + 1];
    }
    [previous, row] = [row, previous];
  }
  return { width, height, samples };
}

export interface Province {
  id: number;
  key: string;
  kind: 'land' | 'sea' | 'lake';
  terrain: string;
  centroid: [number, number];
  neighbors: { id: number; edge: string }[];
}

export interface MapAssets {
  provinces: Map<number, Province>;
  raster: Raster;
  heightmap: Heightmap;
  terrain: Raster;
  rivers: Raster;
}

export async function loadMapAssets(base: string, signal: AbortSignal): Promise<MapAssets> {
  async function file(name: string): Promise<Response> {
    const response = await fetch(base + name, { signal });
    if (!response.ok) throw new Error('Could not load ' + name + ' (HTTP ' + response.status + ')');
    return response;
  }

  async function raster(name: string): Promise<Raster> {
    const blob = await (await file(name)).blob();
    const bitmap = await createImageBitmap(blob, {
      colorSpaceConversion: 'none',
      premultiplyAlpha: 'none',
    });
    try {
      const canvas = document.createElement('canvas');
      canvas.width = bitmap.width;
      canvas.height = bitmap.height;
      const context = canvas.getContext('2d', { willReadFrequently: true });
      if (!context) throw new Error('Could not read ' + name);
      context.drawImage(bitmap, 0, 0);
      return context.getImageData(0, 0, bitmap.width, bitmap.height);
    } finally {
      bitmap.close();
    }
  }

  const [metadata, provincesRaster, heightmap, terrain, rivers] = await Promise.all([
    file('provinces.json').then((response) => response.json()) as Promise<{ provinces: Province[] }>,
    raster('provinces.png'),
    file('heightmap.png').then((response) => response.arrayBuffer()).then(decodeHeightmap16),
    raster('terrain.png'),
    raster('rivers.png'),
  ]);
  const { width, height } = provincesRaster;
  if ([heightmap, terrain, rivers].some((layer) => layer.width !== width || layer.height !== height)) {
    throw new Error('Map layers must have the same dimensions');
  }

  const provinces = new Map(metadata.provinces.map((province) => [province.id, province]));
  if (provinces.size !== metadata.provinces.length || provinces.size === 0
    || [...provinces.keys()].some((id) => !Number.isInteger(id) || id <= 0 || id > 0xffffff)) {
    throw new Error('Map metadata has invalid RGB province IDs');
  }
  return { provinces, raster: provincesRaster, heightmap, terrain, rivers };
}
