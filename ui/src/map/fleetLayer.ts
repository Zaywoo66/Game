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
}

const transitionMs = 180;

export function createFleetLayer(
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

  function position(motion: FleetMotion, now: number, output: THREE.Vector3) {
    return output.lerpVectors(motion.from, motion.to, THREE.MathUtils.clamp((now - motion.started) / transitionMs, 0, 1));
  }

  function setMarkers(markers: readonly FleetMarker[]) {
    const now = performance.now();
    const next = new Map<number, FleetMotion>();
    const stacks = new Map<number, number>();
    for (const marker of markers) {
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
      next.set(marker.id, { from, to, destination, started: previous && previous.to.equals(to) ? previous.started : now });
      let badge = badges.get(marker.id);
      if (!badge) {
        badge = document.createElement('button');
        badge.type = 'button';
        badge.className = 'map-fleet-badge';
        badge.dataset.fleetId = String(marker.id);
        badge.addEventListener('click', () => {
          selected = marker.id;
          update(performance.now());
          onSelect(marker.id);
        });
        overlay.appendChild(badge);
        badges.set(marker.id, badge);
      }
      badge.textContent = `⚓ ${marker.ships.toLocaleString('en-US')}`;
      badge.setAttribute('aria-label', `Fleet ${marker.id}, ${marker.ships} ships`);
      badge.title = `Fleet ${marker.id} · ${marker.ships.toLocaleString('en-US')} ships`;
      badge.style.setProperty('--fleet-color', /^#[a-f\d]{6}$/i.test(marker.color) ? marker.color : '#ffffff');
    }
    for (const [id, badge] of badges) {
      if (!next.has(id)) { badge.remove(); badges.delete(id); }
    }
    motions = next;
    viewport.dataset.mapFleetInstances = String(next.size);
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
  }

  return {
    setMarkers,
    select(id: number) { selected = id; update(performance.now()); },
    update,
    dispose() { overlay.remove(); badges.clear(); motions.clear(); },
  };
}
