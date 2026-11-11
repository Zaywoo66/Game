import * as THREE from 'three';
import { OrbitControls } from 'three/addons/controls/OrbitControls.js';
import { ArmyMarker, createArmyLayer } from './armyLayer';
import { decodeProvinceId, MapAssets, pickProvinceId, Raster, Rgba, updateOwnerLookup } from './mapData';
import { historicalPresentation, MapMode } from './mapPresentation';
import { mapFragmentShader, mapVertexShader, oceanFragmentShader, oceanVertexShader } from './mapShaders';
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
  setCameraPreset(name: 'whole-region' | 'britain' | 'iberia' | 'province'): void;
  readCamera(): { target: [number, number]; zoom: number; polarAngle: number };
  readPixel(clientX: number, clientY: number): Rgba;
  setMapMode(mode: MapMode): void;
  readMapMode(): MapMode;
  focusProvince(id: number): void;
}

interface MapViewer {
  renderer: string;
  setOwnerColors(colors: ReadonlyMap<number, Rgba>): void;
  setOccupationColors(colors: ReadonlyMap<number, Rgba>): void;
  setArmies(armies: readonly ArmyMarker[]): void;
  setArmySelection(id: number, targeting: boolean): void;
  setMapMode(mode: MapMode): void;
  setCountryLabels(labels: readonly { id: number; text: string; capital: boolean }[]): void;
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
  const areaBytes = new Uint8Array(lookupPixels * 4);
  const historicalBytes = new Uint8Array(lookupPixels * 4);
  const precisionBytes = new Uint8Array(lookupPixels);
  const historical = historicalPresentation(assets);
  const areaKeys = [...new Set([...provinces.values()].map((province) => province.area).filter((area): area is string => Boolean(area)))].sort();
  const areaIds = new Map(areaKeys.map((area, index) => [area, index + 1]));
  waterBytes[0] = 1;
  for (const [id, province] of provinces) {
    if (province.kind === 'sea') waterBytes[id] = 1;
    if (province.kind === 'lake') waterBytes[id] = 2;
    const area = province.area ? areaIds.get(province.area) ?? 0 : 0;
    areaBytes[id * 4] = area >> 16;
    areaBytes[id * 4 + 1] = (area >> 8) & 0xff;
    areaBytes[id * 4 + 2] = area & 0xff;
    areaBytes[id * 4 + 3] = 255;
    const historicColor = historical.colors.get(id);
    if (historicColor) historicalBytes.set(historicColor, id * 4);
    precisionBytes[id] = historical.precision.get(id) ?? 0;
  }

  const provinceTexture = texture(new Uint8Array(data.buffer, data.byteOffset, data.byteLength), width, height);
  const terrainTexture = texture(redChannel(assets.terrain), width, height, THREE.RedFormat);
  const riverTexture = texture(redChannel(assets.rivers), width, height, THREE.RedFormat);
  const ownerTexture = texture(ownerBytes, lookupWidth, lookupHeight);
  ownerTexture.colorSpace = THREE.SRGBColorSpace;
  const occupationTexture = texture(occupationBytes, lookupWidth, lookupHeight);
  occupationTexture.colorSpace = THREE.SRGBColorSpace;
  const waterTexture = texture(waterBytes, lookupWidth, lookupHeight, THREE.RedFormat);
  const areaTexture = texture(areaBytes, lookupWidth, lookupHeight);
  const historicalTexture = texture(historicalBytes, lookupWidth, lookupHeight);
  historicalTexture.colorSpace = THREE.SRGBColorSpace;
  const precisionTexture = texture(precisionBytes, lookupWidth, lookupHeight, THREE.RedFormat);
  const heightTexture = new THREE.DataTexture(assets.heightmap.samples, width, height, THREE.RedFormat, THREE.UnsignedShortType);
  heightTexture.magFilter = THREE.NearestFilter;
  heightTexture.minFilter = THREE.NearestFilter;
  heightTexture.generateMipmaps = false;
  heightTexture.unpackAlignment = 1;
  heightTexture.needsUpdate = true;
  const textures = [provinceTexture, terrainTexture, riverTexture, ownerTexture, occupationTexture, waterTexture, areaTexture, historicalTexture, precisionTexture, heightTexture];
  const uniforms = {
    provinceRaster: { value: provinceTexture },
    terrainRaster: { value: terrainTexture },
    riverRaster: { value: riverTexture },
    ownerLookup: { value: ownerTexture },
    historicalLookup: { value: historicalTexture },
    precisionLookup: { value: precisionTexture },
    occupationLookup: { value: occupationTexture },
    waterLookup: { value: waterTexture },
    areaLookup: { value: areaTexture },
    heightRaster: { value: heightTexture },
    rasterSize: { value: new THREE.Vector2(width, height) },
    lookupSize: { value: new THREE.Vector2(lookupWidth, lookupHeight) },
    selectedId: { value: 0 },
    hasOccupations: { value: false },
    mapMode: { value: 0 },
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
  let mapMode: MapMode = 'political';
  function setMapMode(mode: MapMode) {
    mapMode = mode;
    uniforms.mapMode.value = mode === 'political' ? 0 : mode === 'terrain' ? 1 : 2;
    lastLabelRender = -Infinity;
    renderFrame(performance.now());
  }

  // Open water beyond the finite regional raster keeps wide windows and
  // tilted views from exposing the scene background at the map edge.
  const oceanGeometry = new THREE.PlaneGeometry(width * 5, height * 5);
  oceanGeometry.rotateX(-Math.PI / 2);
  oceanGeometry.translate(width / 2, -1, height / 2);
  const oceanMaterial = new THREE.ShaderMaterial({
    uniforms: { rasterSize: uniforms.rasterSize, seconds: uniforms.seconds },
    vertexShader: oceanVertexShader,
    fragmentShader: oceanFragmentShader,
    side: THREE.DoubleSide,
  });
  scene.add(new THREE.Mesh(oceanGeometry, oceanMaterial));

  const span = Math.max(width, height);
  const camera = new THREE.OrthographicCamera(-span / 2, span / 2, span / 2, -span / 2, 0.1, span * 8);
  const controls = new OrbitControls(camera, renderer.domElement);
  controls.target.set(width / 2, 0, height / 2);
  controls.minZoom = 0.85;
  controls.maxZoom = 6;
  controls.minPolarAngle = 0.02;
  controls.maxPolarAngle = 0.72;
  controls.enableDamping = true;
  controls.dampingFactor = 0.14;
  controls.zoomToCursor = false;
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
    const frameHeight = height * 1.08;
    camera.left = -frameHeight * w / h / 2;
    camera.right = -camera.left;
    camera.top = frameHeight / 2;
    camera.bottom = -camera.top;
    camera.updateProjectionMatrix();

    if (!fitted) {
      const distance = span * 1.6;
      const tilt = 0.12;
      camera.position.copy(controls.target).add(new THREE.Vector3(
        0,
        distance * Math.cos(tilt),
        distance * Math.sin(tilt),
      ));
      camera.zoom = manualFrames ? 1 : 1.6;
      camera.updateProjectionMatrix();
      fitted = true;
    }
    controls.update();
    camera.updateMatrixWorld();
  }

  viewport.appendChild(renderer.domElement);
  const labelLayer = document.createElement('div');
  labelLayer.className = 'map-label-layer';
  labelLayer.setAttribute('aria-hidden', 'true');
  viewport.appendChild(labelLayer);
  let countryLabels: readonly { id: number; text: string; capital: boolean }[] = [];
  let lastLabelRender = -Infinity;
  function renderLabels(now: number) {
    if (now - lastLabelRender < 90) return;
    lastLabelRender = now;
    const bounds = viewport.getBoundingClientRect();
    const labels = mapMode === 'historical'
      ? historical.labels.map((item) => ({ ...item, capital: false }))
      : countryLabels;
    const candidates = camera.zoom < 2.2
      ? mapMode === 'terrain' ? [] : labels
      : [...provinces.values()].filter((province) => province.kind === 'land').map((province) => ({
        id: province.id,
        text: assets.names.get(province.name_key ?? '') ?? `Province ${province.id}`,
        capital: countryLabels.some((country) => country.id === province.id && country.capital),
      }));
    const placed: { x: number; y: number; halfWidth: number }[] = [];
    const nodes: HTMLElement[] = [];
    for (const item of candidates) {
      if (nodes.length >= (camera.zoom < 1.5 ? 24 : 70)) break;
      const point = projectProvince(item.id);
      if (!point) continue;
      const x = point.x - bounds.left;
      const y = point.y - bounds.top;
      const halfWidth = Math.max(55, item.text.length * (camera.zoom < 2.2 ? 4.9 : 3.7));
      if (x < halfWidth || y < 25 || x > bounds.width - halfWidth || y > bounds.height - 25
        || (x < 350 && y < 175)
        || placed.some((previous) => Math.abs(previous.x - x) < previous.halfWidth + halfWidth + 8 && Math.abs(previous.y - y) < 29)) continue;
      placed.push({ x, y, halfWidth });
      const label = document.createElement('span');
      label.className = item.capital ? 'map-label map-label--capital' : 'map-label';
      label.dataset.kind = camera.zoom < 2.2 ? 'country' : item.capital ? 'capital' : 'province';
      label.textContent = item.capital ? `★ ${item.text}` : item.text;
      label.style.left = `${x}px`;
      label.style.top = `${y}px`;
      nodes.push(label);
    }
    labelLayer.replaceChildren(...nodes);
  }
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

  const armyLayer = createArmyLayer(scene, viewport, provinceAnchor, camera);
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
    const halfWidth = (camera.right - camera.left) / (2 * camera.zoom);
    const halfHeight = (camera.top - camera.bottom) / (2 * camera.zoom);
    const horizontalPan = Math.max(0, width / 2 - halfWidth * 0.7);
    const verticalPan = Math.max(0, height / 2 - halfHeight * 0.7);
    const boundedX = THREE.MathUtils.clamp(controls.target.x, width / 2 - horizontalPan, width / 2 + horizontalPan);
    const boundedZ = THREE.MathUtils.clamp(controls.target.z, height / 2 - verticalPan, height / 2 + verticalPan);
    camera.position.x += boundedX - controls.target.x;
    camera.position.z += boundedZ - controls.target.z;
    controls.target.x = boundedX;
    controls.target.z = boundedZ;
    uniforms.seconds.value = manualFrames ? 0 : (now - start) / 1_000;
    armyLayer.update(now);
    renderer.render(scene, camera);
    renderLabels(now);

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

  function setCameraPreset(name: 'whole-region' | 'britain' | 'iberia' | 'province') {
    const london = [...provinces.values()].find((province) => province.key === 'london');
    const presets = {
      'whole-region': { x: width / 2, z: height / 2, zoom: 1, tilt: 0.12 },
      britain: { x: width * 0.39, z: height * 0.27, zoom: 2.1, tilt: 0.15 },
      iberia: { x: width * 0.38, z: height * 0.72, zoom: 2.1, tilt: 0.15 },
      province: { x: london?.centroid[0] ?? width / 2, z: london?.centroid[1] ?? height / 2, zoom: 4.2, tilt: 0.23 },
    };
    const preset = presets[name];
    const distance = span * 1.6;
    controls.target.set(preset.x, 0, preset.z);
    camera.position.copy(controls.target).add(new THREE.Vector3(0, distance * Math.cos(preset.tilt), distance * Math.sin(preset.tilt)));
    camera.zoom = preset.zoom;
    camera.updateProjectionMatrix();
    controls.update();
    lastLabelRender = -Infinity;
    renderFrame(performance.now());
  }

  function focusProvince(id: number) {
    const center = provinceAnchor(id);
    if (!center) return;
    controls.target.set(center.x, 0, center.z);
    camera.position.set(center.x, span * 1.6, center.z + span * 0.25);
    camera.zoom = 2.5;
    camera.updateProjectionMatrix();
    controls.update();
    lastLabelRender = -Infinity;
    renderFrame(performance.now());
  }

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
    setMapMode,
    setCountryLabels(labels) { countryLabels = labels; lastLabelRender = -Infinity; renderLabels(performance.now()); },
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
      setCameraPreset,
      readCamera: () => ({ target: [controls.target.x, controls.target.z], zoom: camera.zoom, polarAngle: controls.getPolarAngle() }),
      readPixel(clientX, clientY) {
        renderFrame(performance.now());
        const rect = renderer.domElement.getBoundingClientRect();
        const x = Math.floor((clientX - rect.left) / rect.width * context.drawingBufferWidth);
        const y = Math.floor((rect.bottom - clientY) / rect.height * context.drawingBufferHeight);
        const pixel = new Uint8Array(4);
        context.readPixels(x, y, 1, 1, context.RGBA, context.UNSIGNED_BYTE, pixel);
        return [pixel[0], pixel[1], pixel[2], pixel[3]];
      },
      setMapMode,
      readMapMode: () => mapMode,
      focusProvince,
    },
    dispose() {
      cancelAnimationFrame(frame);
      observer.disconnect();
      controls.dispose();
      renderer.domElement.removeEventListener('pointerdown', pointerDown);
      renderer.domElement.removeEventListener('pointerup', pointerUp);
      renderer.domElement.removeEventListener('pointercancel', cancelPointer);
      renderer.domElement.remove();
      labelLayer.remove();
      armyLayer.dispose();
      chunks.forEach(({ geometry }) => geometry.dispose());
      oceanGeometry.dispose();
      oceanMaterial.dispose();
      textures.forEach((value) => value.dispose());
      material.dispose();
      renderer.dispose();
    },
  };
}
