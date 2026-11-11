import * as THREE from 'three';
import { GLTFLoader } from 'three/addons/loaders/GLTFLoader.js';

export interface ArmyMarker {
  id: number;
  province: number;
  destination: number | null;
  progress: number;
  color: string;
}

interface Motion {
  from: THREE.Vector3;
  to: THREE.Vector3;
  angle: number;
  color: THREE.Color;
  started: number;
}

const transitionMs = 180;
const markerScale = 12;

export function createArmyLayer(
  scene: THREE.Scene,
  viewport: HTMLElement,
  provinceAnchor: (id: number) => THREE.Vector3 | null,
) {
  let disposed = false;
  let requested = false;
  let geometry: THREE.BufferGeometry | undefined;
  let material: THREE.MeshStandardMaterial | undefined;
  let mesh: THREE.InstancedMesh | undefined;
  let capacity = 0;
  let selected = 0;
  let dirty = false;
  let ids: number[] = [];
  let motions = new Map<number, Motion>();
  const dummy = new THREE.Object3D();
  dummy.scale.setScalar(markerScale);
  const white = new THREE.Color('#ffffff');
  const selectedColor = new THREE.Color('#ffe2a0');
  const light = new THREE.HemisphereLight('#f7f3e8', '#546b61', 2.1);
  scene.add(light);

  function rebuildInstances() {
    if (!geometry || !material) return;
    if (ids.length > capacity) {
      if (mesh) {
        scene.remove(mesh);
        mesh.dispose();
      }
      capacity = 2 ** Math.ceil(Math.log2(ids.length));
      mesh = new THREE.InstancedMesh(geometry, material, capacity);
      mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
      mesh.frustumCulled = false;
      scene.add(mesh);
    }
    if (mesh) mesh.count = ids.length;
    viewport.dataset.mapArmyInstances = String(ids.length);
    dirty = true;
  }

  function loadModel() {
    if (requested) return;
    requested = true;
    void new GLTFLoader().loadAsync(import.meta.env.BASE_URL + 'models/infantry.glb')
      .then(({ scene: model }) => {
        const parts: THREE.Mesh[] = [];
        model.traverse((child) => {
          if (child instanceof THREE.Mesh) parts.push(child);
        });
        const source = parts[0];
        if (parts.length !== 1 || !(source.material instanceof THREE.MeshStandardMaterial)) {
          throw new Error('Infantry model must contain one mesh with a shared palette');
        }
        if (disposed) {
          source.geometry.dispose();
          source.material.map?.dispose();
          source.material.dispose();
          return;
        }
        model.updateMatrixWorld(true);
        geometry = source.geometry.applyMatrix4(source.matrixWorld);
        material = source.material;
        rebuildInstances();
      })
      .catch((error: unknown) => {
        if (disposed) return;
        viewport.dataset.mapArmyError = 'true';
        console.error('Could not load infantry model:', error);
      });
  }

  function currentPosition(motion: Motion, now: number, output: THREE.Vector3) {
    const blend = THREE.MathUtils.clamp((now - motion.started) / transitionMs, 0, 1);
    return output.lerpVectors(motion.from, motion.to, blend);
  }

  function setMarkers(markers: readonly ArmyMarker[]) {
    const next = new Map<number, Motion>();
    const now = performance.now();
    const stacks = new Map<number, number>();
    for (const marker of markers) {
      const anchor = provinceAnchor(marker.province);
      if (!anchor) continue;
      const target = marker.destination === null ? null : provinceAnchor(marker.destination);
      const progress = THREE.MathUtils.clamp(marker.progress / 1000, 0, 1);
      const to = target ? anchor.clone().lerp(target, progress) : anchor.clone();
      to.y += 1.5;
      if (!target) {
        const index = stacks.get(marker.province) ?? 0;
        stacks.set(marker.province, index + 1);
        if (index > 0) {
          const angle = index * 2.399963;
          const spread = 12 * Math.sqrt(index);
          to.x += Math.sin(angle) * spread;
          to.z += Math.cos(angle) * spread;
        }
      }
      const color = /^#[a-f\d]{6}$/i.test(marker.color) ? new THREE.Color(marker.color) : white.clone();
      color.lerp(white, 0.28);
      const previous = motions.get(marker.id);
      const angle = target ? Math.atan2(target.x - anchor.x, target.z - anchor.z) : previous?.angle ?? 0;
      if (previous && previous.to.equals(to) && previous.color.equals(color) && previous.angle === angle) {
        next.set(marker.id, previous);
      } else {
        next.set(marker.id, {
          from: previous ? currentPosition(previous, now, new THREE.Vector3()) : to.clone(),
          to, angle, color, started: now,
        });
      }
    }
    motions = next;
    ids = [...next.keys()];
    if (ids.length > 0) loadModel();
    rebuildInstances();
  }

  function update(now: number) {
    if (!mesh || !ids.length || !dirty) return;
    // Keep one final update after crossing the interpolation end.
    let moving = false;
    for (let index = 0; index < ids.length; index++) {
      const motion = motions.get(ids[index]);
      if (!motion) continue;
      currentPosition(motion, now, dummy.position);
      dummy.rotation.y = motion.angle;
      dummy.updateMatrix();
      mesh.setMatrixAt(index, dummy.matrix);
      mesh.setColorAt(index, ids[index] === selected ? selectedColor : motion.color);
      if (now - motion.started < transitionMs) moving = true;
    }
    mesh.boundingSphere = null;
    mesh.instanceMatrix.needsUpdate = true;
    if (mesh.instanceColor) mesh.instanceColor.needsUpdate = true;
    dirty = moving;
  }

  return {
    setMarkers,
    update,
    select(id: number) {
      if (selected !== id) {
        selected = id;
        dirty = true;
      }
    },
    pick(raycaster: THREE.Raycaster): number {
      if (!mesh || !ids.length) return 0;
      const instance = raycaster.intersectObject(mesh, false)[0]?.instanceId;
      return instance === undefined ? 0 : ids[instance] ?? 0;
    },
    readPosition(id: number): [number, number, number] | null {
      const index = ids.indexOf(id);
      if (!mesh || index === -1) return null;
      const matrix = new THREE.Matrix4();
      mesh.getMatrixAt(index, matrix);
      return new THREE.Vector3().setFromMatrixPosition(matrix).toArray();
    },
    dispose() {
      disposed = true;
      scene.remove(light);
      if (mesh) {
        scene.remove(mesh);
        mesh.dispose();
      }
      geometry?.dispose();
      material?.map?.dispose();
      material?.dispose();
    },
  };
}
