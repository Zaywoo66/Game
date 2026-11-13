const openOceanColor = `
vec3 openOcean(vec2 uv, float time) {
  float swell = sin(dot(uv, vec2(82.0, 55.0)) + time * 0.38)
    * sin(dot(uv, vec2(47.0, -69.0)) - time * 0.24);
  return vec3(0.018, 0.048, 0.078) + swell * vec3(0.0012, 0.0024, 0.003);
}
`;

export const oceanVertexShader = `
uniform vec2 rasterSize;
varying vec2 oceanUv;
void main() {
  oceanUv = position.xz / rasterSize;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

export const oceanFragmentShader = `
uniform float seconds;
varying vec2 oceanUv;
${openOceanColor}
void main() {
  gl_FragColor = vec4(openOcean(oceanUv, seconds), 1.0);
  #include <colorspace_fragment>
}
`;

export const mapVertexShader = `
varying vec2 mapUv;
varying vec3 mapNormal;
varying float mapHeight;

void main() {
  mapUv = uv;
  mapNormal = normal;
  mapHeight = position.y;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

export const mapFragmentShader = `
${openOceanColor}
uniform sampler2D provinceRaster;
uniform sampler2D terrainRaster;
uniform sampler2D riverRaster;
uniform sampler2D ownerLookup;
uniform sampler2D historicalLookup;
uniform sampler2D precisionLookup;
uniform sampler2D occupationLookup;
uniform sampler2D waterLookup;
uniform sampler2D areaLookup;
uniform sampler2D heightRaster;
uniform vec2 rasterSize;
uniform vec2 lookupSize;
uniform float selectedId;
uniform float hoveredId;
uniform float seconds;
uniform bool hasOccupations;
uniform int mapMode;

varying vec2 mapUv;
varying vec3 mapNormal;
varying float mapHeight;

float provinceAt(vec2 point) {
  vec3 rgb = floor(texture2D(provinceRaster, point).rgb * 255.0 + 0.5);
  return dot(rgb, vec3(65536.0, 256.0, 1.0));
}

vec2 organicMapUv(vec2 uv) {
  float bendX = 3.75 * sin(uv.y * 41.0 + sin(uv.y * 13.0) * 1.8)
    + 1.65 * sin(uv.y * 107.0 + 0.7);
  float bendY = 3.45 * sin(uv.x * 37.0 + sin(uv.x * 12.0) * 1.6)
    + 1.5 * sin(uv.x * 91.0 + 1.1);
  return clamp(uv + vec2(bendX / rasterSize.x, bendY / rasterSize.y), vec2(0.0), vec2(1.0));
}

vec2 lookupAt(float id) {
  return (vec2(mod(id, lookupSize.x), floor(id / lookupSize.x)) + 0.5) / lookupSize;
}

float areaAt(float id) {
  vec3 rgb = floor(texture2D(areaLookup, lookupAt(id)).rgb * 255.0 + 0.5);
  return dot(rgb, vec3(65536.0, 256.0, 1.0));
}

vec3 boundariesAt(vec2 point, float id, float area, vec4 owner) {
  float neighbor = provinceAt(point);
  float province = step(0.5, abs(id - neighbor));
  float adjacentArea = areaAt(neighbor);
  float region = province * step(0.5, min(area, adjacentArea)) * step(0.5, abs(area - adjacentArea));
  vec4 adjacentOwner = mapMode == 2
    ? texture2D(historicalLookup, lookupAt(neighbor))
    : texture2D(ownerLookup, lookupAt(neighbor));
  float country = province * step(0.06, length(owner.rgb - adjacentOwner.rgb) + abs(owner.a - adjacentOwner.a));
  return vec3(province, region, country);
}

void main() {
  vec2 sampleUv = organicMapUv(mapUv);
  float id = provinceAt(sampleUv);
  vec2 lookup = lookupAt(id);
  float water = texture2D(waterLookup, lookup).r * 255.0;
  vec4 owner = texture2D(ownerLookup, lookup);
  vec4 historical = texture2D(historicalLookup, lookup);
  vec4 borderOwner = mapMode == 2 ? historical : owner;
  float sourceResolution = texture2D(precisionLookup, lookup).r * 255.0;
  float terrain = texture2D(terrainRaster, sampleUv).r * 255.0;
  float river = texture2D(riverRaster, sampleUv).r;

  vec2 pixel = sampleUv * rasterSize;
  vec3 color;
  float landLight = 0.0;
  if (water > 0.5) {
    vec2 encodedHeight = texture2D(heightRaster, sampleUv).rg;
    float elevation = (encodedHeight.r * 65280.0 + encodedHeight.g * 255.0) / 65535.0;
    float depth = clamp((0.5 - elevation) * 7.0, 0.0, 1.0);
    color = mix(vec3(0.055, 0.118, 0.160), vec3(0.018, 0.048, 0.078), smoothstep(0.02, 0.80, depth));
    float shelf = (1.0 - smoothstep(0.02, 0.13, depth)) * 0.16;
    color = mix(color, vec3(0.105, 0.165, 0.188), shelf);
    if (water > 1.5) color = mix(color, vec3(0.075, 0.133, 0.165), 0.28);
    float swell = sin(dot(mapUv, vec2(82.0, 55.0)) + seconds * 0.38)
      * sin(dot(mapUv, vec2(47.0, -69.0)) - seconds * 0.24);
    color += swell * vec3(0.001, 0.002, 0.0035);
  } else {
    color = vec3(0.245, 0.232, 0.145);
    if (terrain > 1.5) color = vec3(0.310, 0.278, 0.177);
    if (terrain > 2.5) color = vec3(0.385, 0.354, 0.276);
    landLight = dot(normalize(mapNormal), normalize(vec3(-0.48, 0.84, -0.25)));
    float altitude = clamp(mapHeight / 25.0, 0.0, 1.0);
    color = mix(color, vec3(0.49, 0.43, 0.30), altitude * 0.22);
    color *= 0.66 + 0.52 * clamp(landLight, 0.0, 1.0);
    color = mix(color, vec3(0.045, 0.165, 0.178), river * 0.74);
  }

  if (mapMode == 0) color = mix(color, owner.rgb, owner.a * 0.88);
  if (mapMode == 2) {
    color = mix(color, historical.rgb, historical.a * 0.88);
  }
  if (mapMode == 2 || (mapMode == 0 && owner.a < 0.99)) {
    if (sourceResolution > 1.5 && sourceResolution < 3.5) {
      float hatch = step(0.94, fract((pixel.x + pixel.y) * 0.13));
      if (sourceResolution > 2.5) {
        color = mix(color, vec3(0.26, 0.23, 0.17), hatch * 0.07);
      } else {
        color = mix(color, vec3(0.26, 0.23, 0.17), hatch * 0.035);
      }
    }
  }
  if (mapMode == 0 && hasOccupations) {
    vec4 occupier = texture2D(occupationLookup, lookup);
    float hatch = step(0.55, fract((pixel.x + pixel.y) * 0.20));
    color = mix(color, occupier.rgb, hatch * occupier.a * 0.76);
  }
  vec2 stepSize = clamp(fwidth(sampleUv) * 0.48, 0.55 / rasterSize, 1.45 / rasterSize);
  float area = areaAt(id);
  vec3 border = max(
    max(boundariesAt(sampleUv + vec2(stepSize.x, 0.0), id, area, borderOwner),
        boundariesAt(sampleUv - vec2(stepSize.x, 0.0), id, area, borderOwner)),
    max(boundariesAt(sampleUv + vec2(0.0, stepSize.y), id, area, borderOwner),
        boundariesAt(sampleUv - vec2(0.0, stepSize.y), id, area, borderOwner))
  );
  vec2 wide = stepSize * 1.7;
  float country = max(border.z, max(
    max(boundariesAt(sampleUv + vec2(wide.x, 0.0), id, area, borderOwner).z,
        boundariesAt(sampleUv - vec2(wide.x, 0.0), id, area, borderOwner).z),
    max(boundariesAt(sampleUv + vec2(0.0, wide.y), id, area, borderOwner).z,
        boundariesAt(sampleUv - vec2(0.0, wide.y), id, area, borderOwner).z)
  ));
  float rasterPixelsPerScreenPixel = max(fwidth(mapUv.x) * rasterSize.x, fwidth(mapUv.y) * rasterSize.y);
  float detail = 1.0 - smoothstep(1.0, 3.5, rasterPixelsPerScreenPixel);
  float borderOpacity = border.x * (0.075 + 0.065 * detail) + border.y * 0.11 + country * 0.75;
  if (water > 0.5) borderOpacity *= 0.035;
  vec3 borderColor = mix(vec3(0.40, 0.38, 0.32), vec3(0.12, 0.14, 0.15), smoothstep(0.05, 0.8, country));
  if (water > 0.5) borderColor = vec3(0.34, 0.43, 0.48);
  color = mix(color, borderColor, clamp(borderOpacity, 0.0, 0.74));
  float selected = selectedId > 0.0 ? 1.0 - step(0.5, abs(id - selectedId)) : 0.0;
  float hovered = hoveredId > 0.0 ? 1.0 - step(0.5, abs(id - hoveredId)) : 0.0;
  float pulse = 0.5 + 0.5 * sin(seconds * 3.2);
  color = mix(color, vec3(0.91, 0.72, 0.39), selected * (0.08 + pulse * 0.04));
  color = mix(color, vec3(0.99, 0.85, 0.58), selected * border.x * 0.78);
  color = mix(color, vec3(0.91, 0.88, 0.75), hovered * border.x * 0.72);
  color = mix(color, vec3(0.94, 0.83, 0.60), hovered * 0.07);

  // Preserve just a touch of relief over opaque political fills so the map
  // remains legible without reverting to the old terrain-colored appearance.
  if (water < 0.5) {
    color *= 0.84 + 0.20 * clamp(landLight, 0.0, 1.0);
    float grain = fract(sin(dot(floor(pixel * 0.62), vec2(12.9898, 78.233))) * 43758.5453);
    color *= 0.985 + grain * 0.025;
  }

  float edgeDistance = min(min(mapUv.x, 1.0 - mapUv.x), min(mapUv.y, 1.0 - mapUv.y));
  float edgeFade = 1.0 - smoothstep(0.0, 0.016, edgeDistance);
  color = mix(color, openOcean(mapUv, seconds), edgeFade);

  gl_FragColor = vec4(color, 1.0);
  #include <colorspace_fragment>
}
`;
