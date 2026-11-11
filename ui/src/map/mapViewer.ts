import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { ArmyMarker, createArmyLayer } from './armyLayer';
import { decodeProvinceId, MapAssets, pickProvinceId, Raster, Rgba, updateOwnerLookup } from './mapData';
import { mapFragmentShader, mapVertexShader } from './mapShaders';
import { createTerrain } from './terrain';

interface OwnerUpdate {
  before: Rgba;
  after: Rgba;
  rasterUnchanged: boolean;
  ownerTextureUnchanged: boolean;
}

export interface BenchmarkSample {
  frameMs: number[];
  drawingBufferWidth: number;
  drawingBufferHeight: number;
}

export interface MapTestControls {
  renderFrame(): void;
  decodeProvinceId: typeof decodeProvinceId;
  pickAt(clientX: number, clientY: number): number;
  projectProvince(id: number): { x: number; y: number } | null;
  readOwnerColor(id: number): Rgba;
  readArmyPosition(id: number): [number, number, number] | null;
  setOwnerColor(id: number, rgba: Rgba): OwnerUpdate;
  startBenchmark(): void;
  readBenchmark(): BenchmarkSample;
}

interface MapViewer {
  renderer: string;
  setOwnerColors(colors: ReadonlyMap<number, Rgba>): void;
  setOccupationColors(colors: ReadonlyMap<number, Rgba>): void;
  setArmies(armies: readonly ArmyMarker[]): void;
  setArmySelection(id: number, targeting: boolean): void;
  startBenchmark(): void;
  readBenchmark(): BenchmarkSample;
  testControls: MapTestControls;
  dispose(): void;
}

function texture(
  bytes: Uint8Array,
  width: number,
  height: number,
  format: typeof THREE.RGBAFormat | typeof THREE.RedFormat = THREE.RGBAFormat,
): THREE.DataTexture {
  const result = new THREE.DataTexture(bytes, width, height, format);
  result.magFilter = THREE.NearestFilter;
  result.minFilter = THREE.NearestFilter;
  result.generateMipmaps = false;
  result.unpackAlignment = 1;
  result.needsUpdate = true;
  return result;
}

function redChannel(raster: Raster): Uint8Array {
  const bytes = new Uint8Array(raster.width * raster.height);
  for (let pixel = 0; pixel < bytes.length; pixel++) {
    bytes[pixel] = raster.data[pixel * 4];
  }
  return bytes;
}

export function createMapViewer(
  viewport: HTMLElement,
  assets: MapAssets,
  onSelect: (id: number) => void,
  onArmySelect: (id: number) => void,
  manualFrames = false,
): MapViewer {
  const { width, height, data } = assets.raster;
  const { provinces } = assets;
  const renderer = new THREE.WebGLRenderer({ powerPreference: 'low-power' });
  renderer.setClearColor('#15232c');
  renderer.domElement.tabIndex = 0;
  renderer.domElement.setAttribute('aria-label', 'Interactive province map');

  const context = renderer.getContext();
  const debug = context.getExtension('WEBGL_debug_renderer_info');
  const gpu = String(context.getParameter(debug?.UNMASKED_RENDERER_WEBGL ?? context.RENDERER));
  console.info('WebGL renderer/GPU: ' + gpu + '; version: ' + context.getParameter(context.VERSION));

  const highestId = [...provinces.keys()].reduce((highest, id) => Math.max(highest, id), 0);
  const lookupWidth = Math.min(renderer.capabilities.maxTextureSize, 1024, highestId + 1);
  const lookupHeight = Math.ceil((highestId + 1) / lookupWidth);
  if (lookupHeight > renderer.capabilities.maxTextureSize) {
    renderer.dispose();
    throw new Error('Province lookup is too large for this GPU');
  }

  const lookupPixels = lookupWidth * lookupHeight;
  const ownerBytes = new Uint8Array(lookupPixels * 4);
  const occupationBytes = new Uint8Array(lookupPixels * 4);
  const waterBytes = new Uint8Array(lookupPixels);
  waterBytes[0] = 1;
  for (const [id, province] of provinces) {
    if (province.kind === 'sea') waterBytes[id] = 1;
    if (province.kind === 'lake') waterBytes[id] = 2;
  }

  const provinceTexture = texture(new Uint8Array(data.buffer, data.byteOffset, data.byteLength), width, height);
  const terrainTexture = texture(redChannel(assets.terrain), width, height, THREE.RedFormat);
  const riverTexture = texture(redChannel(assets.rivers), width, height, THREE.RedFormat);
  const ownerTexture = texture(ownerBytes, lookupWidth, lookupHeight);
  ownerTexture.colorSpace = THREE.SRGBColorSpace;
  const occupationTexture = texture(occupationBytes, lookupWidth, lookupHeight);
  occupationTexture.colorSpace = THREE.SRGBColorSpace;
  const waterTexture = texture(waterBytes, lookupWidth, lookupHeight, THREE.RedFormat);
  const textures = [provinceTexture, terrainTexture, riverTexture, ownerTexture, occupationTexture, waterTexture];
  const uniforms = {
    provinceRaster: { value: provinceTexture },
    terrainRaster: { value: terrainTexture },
    riverRaster: { value: riverTexture },
    ownerLookup: { value: ownerTexture },
    occupationLookup: { value: occupationTexture },
    waterLookup: { value: waterTexture },
    rasterSize: { value: new THREE.Vector2(width, height) },
    lookupSize: { value: new THREE.Vector2(lookupWidth, lookupHeight) },
    selectedId: { value: 0 },
    hasOccupations: { value: false },
    seconds: { value: 0 },
  };
  const material = new THREE.ShaderMaterial({
    uniforms,
    vertexShader: mapVertexShader,
    fragmentShader: mapFragmentShader,
  });
  const chunks = createTerrain(assets, material);
  const scene = new THREE.Scene();
  scene.add(...chunks);

  const span = Math.max(width, height);
  const camera = new THREE.PerspectiveCamera(43, 1, Math.max(0.1, span / 2_000), span * 8);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.set(width / 2, 0, height / 2);
  controls.minDistance = span * 0.14;
  controls.maxDistance = span * 4;
  controls.minPolarAngle = 0.10;
  controls.maxPolarAngle = 1.20;
  controls.enableDamping = true;
  controls.dampingFactor = 0.14;
  controls.zoomToCursor = true;
  controls.screenSpacePanning = false;
  controls.mouseButtons = { LEFT: THREE.MOUSE.PAN, MIDDLE: THREE.MOUSE.DOLLY, RIGHT: THREE.MOUSE.ROTATE };
  controls.touches = { ONE: THREE.TOUCH.PAN, TWO: THREE.TOUCH.DOLLY_ROTATE };
  controls.listenToKeyEvents(renderer.domElement);

  let fitted = false;
  function resize() {
    const w = viewport.clientWidth;
    const h = viewport.clientHeight;
    if (w === 0 || h === 0) return;
    renderer.setPixelRatio(Math.min(window.devicePixelRatio, 2));
    renderer.setSize(w, h, false);
    camera.aspect = w / h;
    camera.updateProjectionMatrix();

    if (!fitted) {
      const distance = 0.64 * Math.max(height, width / camera.aspect) / Math.tan(camera.fov * Math.PI / 360);
      const tilt = 0.50;
      camera.position.copy(controls.target).add(new THREE.Vector3(
        0,
        distance * Math.cos(tilt),
        distance * Math.sin(tilt),
      ));
      fitted = true;
    }
    controls.update();
    camera.updateMatrixWorld();
  }

  viewport.appendChild(renderer.domElement);
  const observer = new ResizeObserver(resize);
  observer.observe(viewport);
  resize();

  const raycaster = new THREE.Raycaster();
  function pickAt(clientX: number, clientY: number): number {
    const rect = renderer.domElement.getBoundingClientRect();
    if (rect.width === 0 || rect.height === 0 || clientX < rect.left || clientX > rect.right
      || clientY < rect.top || clientY > rect.bottom) return 0;

    const ndc = new THREE.Vector2(
      ((clientX - rect.left) / rect.width) * 2 - 1,
      1 - ((clientY - rect.top) / rect.height) * 2,
    );
    raycaster.setFromCamera(ndc, camera);
    const uv = raycaster.intersectObjects(chunks, false)[0]?.uv;
    if (!uv) return 0;
    return pickProvinceId(data, width, height, uv.x, uv.y);
  }

  const representatives = new Map<number, [number, number]>();
  function representative(id: number): [number, number] | null {
    const cached = representatives.get(id);
    if (cached) return cached;
    const province = provinces.get(id);
    if (!province) return null;
    const [centerX, centerY] = province.centroid;
    if (pickProvinceId(data, width, height, (centerX + 0.5) / width, (centerY + 0.5) / height) === id) {
      representatives.set(id, province.centroid);
      return province.centroid;
    }

    let bestDistance = Number.POSITIVE_INFINITY;
    let best: [number, number] | null = null;
    for (let i = 0; i < data.length; i += 4) {
      if (decodeProvinceId(data[i], data[i + 1], data[i + 2]) !== id) continue;
      const x = (i / 4) % width;
      const y = Math.floor(i / 4 / width);
      const distance = (x - centerX) ** 2 + (y - centerY) ** 2;
      if (distance < bestDistance) {
        bestDistance = distance;
        best = [x, y];
      }
    }
    if (best) representatives.set(id, best);
    return best;
  }

  const anchors = new Map<number, THREE.Vector3>();
  function provinceAnchor(id: number): THREE.Vector3 | null {
    const existing = anchors.get(id);
    if (existing) return existing;
    const pixel = representative(id);
    if (!pixel) return null;
    const [x, y] = pixel;
    raycaster.set(new THREE.Vector3(x + 0.5, span, y + 0.5), new THREE.Vector3(0, -1, 0));
    const hit = raycaster.intersectObjects(chunks, false)[0];
    if (!hit) return null;
    anchors.set(id, hit.point);
    return hit.point;
  }

  function projectProvince(id: number): { x: number; y: number } | null {
    const anchor = provinceAnchor(id);
    if (!anchor) return null;
    const point = anchor.clone().project(camera);
    if (Math.abs(point.x) > 1 || Math.abs(point.y) > 1 || Math.abs(point.z) > 1) return null;
    const rect = renderer.domElement.getBoundingClientRect();
    return {
      x: rect.left + (point.x + 1) * rect.width / 2,
      y: rect.top + (1 - point.y) * rect.height / 2,
    };
  }

  function readOwnerColor(id: number): Rgba {
    if (!provinces.has(id)) throw new RangeError('Unknown map province');
    return [
      ownerBytes[id * 4],
      ownerBytes[id * 4 + 1],
      ownerBytes[id * 4 + 2],
      ownerBytes[id * 4 + 3],
    ];
  }

  function setOwnerColor(id: number, rgba: Rgba): OwnerUpdate {
    const before = readOwnerColor(id);
    const rasterBefore = uniforms.provinceRaster.value;
    const rasterVersion = provinceTexture.version;
    const rasterData = provinceTexture.image.data;
    const ownerBefore = uniforms.ownerLookup.value;
    const ownerData = ownerTexture.image.data;
    if (updateOwnerLookup(ownerBytes, id, rgba)) ownerTexture.needsUpdate = true;
    return {
      before,
      after: readOwnerColor(id),
      rasterUnchanged: uniforms.provinceRaster.value === rasterBefore
        && provinceTexture.version === rasterVersion && provinceTexture.image.data === rasterData,
      ownerTextureUnchanged: uniforms.ownerLookup.value === ownerBefore
        && ownerTexture.image.data === ownerData,
    };
  }

  function updateLookup(
    bytes: Uint8Array,
    target: THREE.DataTexture,
    colors: ReadonlyMap<number, Rgba>,
  ) {
    let changed = false;
    for (const id of provinces.keys()) {
      if (updateOwnerLookup(bytes, id, colors.get(id) ?? [0, 0, 0, 0])) changed = true;
    }
    if (changed) target.needsUpdate = true;
  }

  const armyLayer = createArmyLayer(scene, viewport, provinceAnchor);
  let targeting = false;
  let press: { x: number; y: number; id: number } | null = null;
  function pointerDown(event: PointerEvent) {
    if (event.button === 0 && event.isPrimary) {
      renderer.domElement.focus({ preventScroll: true });
      press = { x: event.clientX, y: event.clientY, id: event.pointerId };
    } else {
      press = null;
    }
  }
  function pointerUp(event: PointerEvent) {
    if (!press || press.id !== event.pointerId) return;
    const original = press;
    press = null;
    if (Math.hypot(original.x - event.clientX, original.y - event.clientY) > 4) return;
    const id = pickAt(event.clientX, event.clientY);
    const army = id > 0 && !targeting ? armyLayer.pick(raycaster) : 0;
    if (army) {
      onArmySelect(army);
      return;
    }
    uniforms.selectedId.value = id;
    onSelect(id);
  }
  function cancelPointer() {
    press = null;
  }
  renderer.domElement.addEventListener('pointerdown', pointerDown);
  renderer.domElement.addEventListener('pointerup', pointerUp);
  renderer.domElement.addEventListener('pointercancel', cancelPointer);

  let sample: number[] = [];
  let recording = false;
  let previousFrame: number | null = null;
  function startBenchmark() {
    sample = [];
    previousFrame = null;
    recording = true;
  }
  function readBenchmark(): BenchmarkSample {
    recording = false;
    return {
      frameMs: sample.slice(),
      drawingBufferWidth: context.drawingBufferWidth,
      drawingBufferHeight: context.drawingBufferHeight,
    };
  }

  let frame = 0;
  const start = performance.now();
  function renderFrame(now: number) {
    controls.update();
    const boundedX = THREE.MathUtils.clamp(controls.target.x, 0, width);
    const boundedZ = THREE.MathUtils.clamp(controls.target.z, 0, height);
    camera.position.x += boundedX - controls.target.x;
    camera.position.z += boundedZ - controls.target.z;
    controls.target.x = boundedX;
    controls.target.z = boundedZ;
    uniforms.seconds.value = manualFrames ? 0 : (now - start) / 1_000;
    armyLayer.update(now);
    renderer.render(scene, camera);

    if (recording) {
      if (previousFrame !== null && sample.length < 10_000) sample.push(now - previousFrame);
      previousFrame = now;
    }
  }

  function animate(now: number) {
    renderFrame(now);
    frame = requestAnimationFrame(animate);
  }
  if (manualFrames) renderFrame(start);
  else animate(start);

  return {
    renderer: gpu,
    setOwnerColors: (colors) => updateLookup(ownerBytes, ownerTexture, colors),
    setOccupationColors(colors) {
      uniforms.hasOccupations.value = colors.size > 0;
      updateLookup(occupationBytes, occupationTexture, colors);
    },
    setArmies: armyLayer.setMarkers,
    setArmySelection(id, orderTargeting) {
      armyLayer.select(id);
      targeting = orderTargeting;
    },
    startBenchmark,
    readBenchmark,
    testControls: {
      renderFrame: () => renderFrame(performance.now()),
      decodeProvinceId, pickAt, projectProvince, readOwnerColor,
      readArmyPosition(id) {
        armyLayer.update(performance.now());
        return armyLayer.readPosition(id);
      },
      setOwnerColor, startBenchmark, readBenchmark,
    },
    dispose() {
      cancelAnimationFrame(frame);
      observer.disconnect();
      controls.dispose();
      renderer.domElement.removeEventListener('pointerdown', pointerDown);
      renderer.domElement.removeEventListener('pointerup', pointerUp);
      renderer.domElement.removeEventListener('pointercancel', cancelPointer);
      renderer.domElement.remove();
      armyLayer.dispose();
      chunks.forEach(({ geometry }) => geometry.dispose());
      textures.forEach((value) => value.dispose());
      material.dispose();
      renderer.dispose();
    },
  };
}
