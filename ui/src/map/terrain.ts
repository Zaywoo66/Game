import * as THREE from 'three';
import { MapAssets, pickProvinceId } from './mapData';

const sampleStep = 6;
const chunkCells = 32;
const seaLevel = 32768;
const metersToMapUnits = 0.012;

export function createTerrain(
  assets: MapAssets,
  material: THREE.ShaderMaterial,
): THREE.Mesh<THREE.BufferGeometry, THREE.ShaderMaterial>[] {
  const { width, height, data } = assets.raster;
  const { samples } = assets.heightmap;
  const columns = Math.ceil(width / sampleStep);
  const rows = Math.ceil(height / sampleStep);
  const heights = new Float32Array((columns + 1) * (rows + 1));

  for (let row = 0; row <= rows; row++) {
    for (let column = 0; column <= columns; column++) {
      const u = column / columns;
      const v = row / rows;
      const id = pickProvinceId(data, width, height, u, v);
      const x = Math.min(width - 1, Math.floor(u * width));
      const y = Math.min(height - 1, Math.floor(v * height));
      const elevation = samples[y * width + x] - seaLevel;
      const isLand = assets.provinces.get(id)?.kind === 'land';
      heights[row * (columns + 1) + column] = isLand ? Math.max(0, elevation) * metersToMapUnits : 0;
    }
  }

  const meshes = [];
  const cellWidth = width / columns;
  const cellHeight = height / rows;
  const readHeight = (x: number, y: number) => heights[y * (columns + 1) + x];

  for (let firstRow = 0; firstRow < rows; firstRow += chunkCells) {
    for (let firstColumn = 0; firstColumn < columns; firstColumn += chunkCells) {
      const chunkWidth = Math.min(chunkCells, columns - firstColumn);
      const chunkHeight = Math.min(chunkCells, rows - firstRow);
      const vertexCount = (chunkWidth + 1) * (chunkHeight + 1);
      const positions = new Float32Array(vertexCount * 3);
      const normals = new Float32Array(vertexCount * 3);
      const uv = new Float32Array(vertexCount * 2);
      const indices = new Uint16Array(chunkWidth * chunkHeight * 6);
      let index = 0;
      let vertex = 0;

      for (let localRow = 0; localRow <= chunkHeight; localRow++) {
        for (let localColumn = 0; localColumn <= chunkWidth; localColumn++) {
          const column = firstColumn + localColumn;
          const row = firstRow + localRow;
          const u = column / columns;
          const v = row / rows;
          positions.set([u * width, readHeight(column, row), v * height], vertex * 3);
          uv.set([u, v], vertex * 2);

          // The same grid samples are shared across chunk edges.
          const left = Math.max(0, column - 1);
          const right = Math.min(columns, column + 1);
          const north = Math.max(0, row - 1);
          const south = Math.min(rows, row + 1);
          const dx = (readHeight(right, row) - readHeight(left, row)) / ((right - left) * cellWidth);
          const dz = (readHeight(column, south) - readHeight(column, north)) / ((south - north) * cellHeight);
          const inverseLength = 1 / Math.hypot(dx, 1, dz);
          normals.set([-dx * inverseLength, inverseLength, -dz * inverseLength], vertex * 3);

          if (localRow < chunkHeight && localColumn < chunkWidth) {
            const nextRow = vertex + chunkWidth + 1;
            indices.set([vertex, nextRow, vertex + 1, vertex + 1, nextRow, nextRow + 1], index);
            index += 6;
          }
          vertex++;
        }
      }

      const geometry = new THREE.BufferGeometry();
      geometry.setAttribute('position', new THREE.BufferAttribute(positions, 3));
      geometry.setAttribute('normal', new THREE.BufferAttribute(normals, 3));
      geometry.setAttribute('uv', new THREE.BufferAttribute(uv, 2));
      geometry.setIndex(new THREE.BufferAttribute(indices, 1));
      geometry.computeBoundingBox();
      geometry.computeBoundingSphere();
      meshes.push(new THREE.Mesh(geometry, material));
    }
  }

  return meshes;
}
