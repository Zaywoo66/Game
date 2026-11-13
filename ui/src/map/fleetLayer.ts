import * as THREE from 'three';

export interface FleetMarker {
  id: number;
  province: number;
  destination: number | null;
  progress: number;
  color: string;
  ships: number;
}

interface FleetMotion {
  from: THREE.Vector3;
  to: THREE.Vector3;
  destination: THREE.Vector3 | null;
  started: number;
  angle: number;
  color: THREE.Color;
}

const transitionMs = 180;

function triangleGeometry(vertices: number[]): THREE.BufferGeometry {
  const geometry = new THREE.BufferGeometry();
  geometry.setAttribute('position', new THREE.Float32BufferAttribute(vertices, 3));
  geometry.computeVertexNormals();
  return geometry;
}

export function createFleetLayer(
  scene: THREE.Scene,
  viewport: HTMLElement,
  provinceAnchor: (id: number) => THREE.Vector3 | null,
  camera: THREE.Camera,
  onSelect: (id: number) => void,
) {
  const overlay = document.createElement('div');
  overlay.className = 'map-fleet-overlay';
  viewport.appendChild(overlay);
  const path = document.createElement('div');
  path.className = 'map-fleet-path';
  overlay.appendChild(path);
  const badges = new Map<number, HTMLButtonElement>();
  let motions = new Map<number, FleetMotion>();
  let selected = 0;
  let ids: number[] = [];
  let capacity = 0;
  let hullMesh: THREE.InstancedMesh | undefined;
  let sailMesh: THREE.InstancedMesh | undefined;
  let mastMesh: THREE.InstancedMesh | undefined;
  let dirty = false;
  const hullGeometry = triangleGeometry([
    0, 0, -9, -5, 0, 6, 5, 0, 6,
    -5, 0, 6, 0, 0, -9, 0, -1, 5,
    0, -1, 5, 0, 0, -9, 5, 0, 6,
    -5, 0, 6, 0, -1, 5, 5, 0, 6,
  ]);
  const sailGeometry = triangleGeometry([
    0, 2, -4, 2, 10, 2, 0, 2, 5,
  ]);
  const mastGeometry = new THREE.CylinderGeometry(0.3, 0.3, 9, 4);
  mastGeometry.translate(0, 4.5, 0);
  const hullMaterial = new THREE.MeshStandardMaterial({ color: '#ffffff', roughness: 0.85, depthTest: false, depthWrite: false });
  const sailMaterial = new THREE.MeshStandardMaterial({ color: '#f3e8cb', side: THREE.DoubleSide, roughness: 1, depthTest: false, depthWrite: false });
  const mastMaterial = new THREE.MeshStandardMaterial({ color: '#59442f', roughness: 1, depthTest: false, depthWrite: false });
  const dummy = new THREE.Object3D();

  function ensureCapacity(count: number) {
    if (count <= capacity) return;
    capacity = 2 ** Math.ceil(Math.log2(count));
    for (const mesh of [hullMesh, sailMesh, mastMesh]) {
      if (mesh) { scene.remove(mesh); mesh.dispose(); }
    }
    hullMesh = new THREE.InstancedMesh(hullGeometry, hullMaterial, capacity);
    sailMesh = new THREE.InstancedMesh(sailGeometry, sailMaterial, capacity);
    mastMesh = new THREE.InstancedMesh(mastGeometry, mastMaterial, capacity);
    for (const mesh of [hullMesh, sailMesh, mastMesh]) {
      mesh.instanceMatrix.setUsage(THREE.DynamicDrawUsage);
      mesh.frustumCulled = false;
      mesh.renderOrder = 5;
      scene.add(mesh);
    }
  }

  function position(motion: FleetMotion, now: number, output: THREE.Vector3) {
    return output.lerpVectors(motion.from, motion.to, THREE.MathUtils.clamp((now - motion.started) / transitionMs, 0, 1));
  }

  function setMarkers(markers: readonly FleetMarker[]) {
    const now = performance.now();
    const next = new Map<number, FleetMotion>();
    const stacks = new Map<number, number>();
    for (const marker of [...markers].sort((left, right) => left.id - right.id)) {
      const anchor = provinceAnchor(marker.province);
      if (!anchor) continue;
      const target = marker.destination === null ? null : provinceAnchor(marker.destination);
      const destination = target?.clone() ?? null;
      const progress = THREE.MathUtils.clamp(marker.progress / 1000, 0, 1);
      const to = destination ? anchor.clone().lerp(destination, progress) : anchor.clone();
      to.y += 5;
      const stack = stacks.get(marker.province) ?? 0;
      stacks.set(marker.province, stack + 1);
      if (stack > 0) {
        const angle = stack * 2.399963;
        to.x += Math.sin(angle) * 14 * Math.sqrt(stack);
        to.z += Math.cos(angle) * 14 * Math.sqrt(stack);
      }
      const previous = motions.get(marker.id);
      const from = previous ? position(previous, now, new THREE.Vector3()) : to.clone();
      const angle = destination ? Math.atan2(destination.x - anchor.x, destination.z - anchor.z) : previous?.angle ?? 0;
      next.set(marker.id, {
        from, to, destination,
        started: previous && previous.to.equals(to) ? previous.started : now,
        angle,
        color: /^#[a-f\d]{6}$/i.test(marker.color) ? new THREE.Color(marker.color) : new THREE.Color('#ffffff'),
      });
      let badge = badges.get(marker.id);
      if (!badge) {
        badge = document.createElement('button');
        badge.type = 'button';
        badge.className = 'map-fleet-badge';
        badge.dataset.fleetId = String(marker.id);
        const icon = document.createElement('span');
        icon.className = 'map-fleet-icon';
        icon.textContent = '⚓';
        const strength = document.createElement('span');
        strength.className = 'map-fleet-strength';
        badge.append(icon, strength);
        badge.addEventListener('click', () => {
          selected = marker.id;
          update(performance.now());
          onSelect(marker.id);
        });
        overlay.appendChild(badge);
        badges.set(marker.id, badge);
      }
      const strength = badge.querySelector('.map-fleet-strength');
      if (strength) strength.textContent = marker.ships.toLocaleString('en-US');
      badge.setAttribute('aria-label', `Fleet ${marker.id}, ${marker.ships} ships`);
      badge.title = `Fleet ${marker.id} · ${marker.ships.toLocaleString('en-US')} ships`;
      badge.style.setProperty('--fleet-color', /^#[a-f\d]{6}$/i.test(marker.color) ? marker.color : '#ffffff');
    }
    for (const [id, badge] of badges) {
      if (!next.has(id)) { badge.remove(); badges.delete(id); }
    }
    motions = next;
    ids = [...next.keys()];
    ensureCapacity(ids.length);
    if (hullMesh) hullMesh.count = ids.length;
    if (sailMesh) sailMesh.count = ids.length;
    if (mastMesh) mastMesh.count = ids.length;
    dirty = true;
    viewport.dataset.mapFleetInstances = String(next.size);
    viewport.dataset.mapFleetMeshInstances = String(next.size);
  }

  function update(now: number) {
    const rect = viewport.getBoundingClientRect();
    let selectedPoint: THREE.Vector3 | null = null;
    let selectedTarget: THREE.Vector3 | null = null;
    const projected = new THREE.Vector3();
    for (const [id, motion] of motions) {
      const badge = badges.get(id);
      if (!badge) continue;
      position(motion, now, projected).project(camera);
      const visible = Math.abs(projected.x) <= 1 && Math.abs(projected.y) <= 1 && Math.abs(projected.z) <= 1;
      badge.hidden = !visible;
      badge.dataset.selected = String(id === selected);
      if (!visible) continue;
      const x = (projected.x + 1) * rect.width / 2;
      const y = (1 - projected.y) * rect.height / 2;
      badge.style.left = `${x}px`;
      badge.style.top = `${y}px`;
      if (id === selected && motion.destination) {
        selectedPoint = new THREE.Vector3(x, y, 0);
        const target = motion.destination.clone().project(camera);
        selectedTarget = new THREE.Vector3((target.x + 1) * rect.width / 2, (1 - target.y) * rect.height / 2, 0);
      }
    }
    path.hidden = !selectedPoint || !selectedTarget;
    if (selectedPoint && selectedTarget) {
      const delta = selectedTarget.clone().sub(selectedPoint);
      path.style.left = `${selectedPoint.x}px`;
      path.style.top = `${selectedPoint.y}px`;
      path.style.width = `${delta.length()}px`;
      path.style.transform = `rotate(${Math.atan2(delta.y, delta.x)}rad)`;
    }
    if (!dirty || !ids.length || !hullMesh || !sailMesh || !mastMesh) return;
    let moving = false;
    for (let index = 0; index < ids.length; index++) {
      const id = ids[index];
      const motion = motions.get(id);
      if (!motion) continue;
      position(motion, now, dummy.position);
      dummy.rotation.set(0, motion.angle, 0);
      dummy.scale.setScalar(0.4);
      dummy.updateMatrix();
      hullMesh.setMatrixAt(index, dummy.matrix);
      sailMesh.setMatrixAt(index, dummy.matrix);
      mastMesh.setMatrixAt(index, dummy.matrix);
      hullMesh.setColorAt(index, motion.color);
      if (now - motion.started < transitionMs) moving = true;
    }
    hullMesh.instanceMatrix.needsUpdate = true;
    sailMesh.instanceMatrix.needsUpdate = true;
    mastMesh.instanceMatrix.needsUpdate = true;
    if (hullMesh.instanceColor) hullMesh.instanceColor.needsUpdate = true;
    hullMesh.computeBoundingSphere();
    sailMesh.computeBoundingSphere();
    mastMesh.computeBoundingSphere();
    dirty = moving;
  }

  return {
    setMarkers,
    select(id: number) { selected = id; dirty = true; update(performance.now()); },
    readPosition(id: number): [number, number, number] | null {
      const index = ids.indexOf(id);
      if (!hullMesh || index < 0) return null;
      const matrix = new THREE.Matrix4();
      hullMesh.getMatrixAt(index, matrix);
      return new THREE.Vector3().setFromMatrixPosition(matrix).toArray();
    },
    update,
    dispose() {
      overlay.remove();
      badges.clear();
      motions.clear();
      for (const mesh of [hullMesh, sailMesh, mastMesh]) if (mesh) { scene.remove(mesh); mesh.dispose(); }
      hullGeometry.dispose(); sailGeometry.dispose(); mastGeometry.dispose();
      hullMaterial.dispose(); sailMaterial.dispose(); mastMaterial.dispose();
    },
  };
}
