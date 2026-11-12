const openOceanColor = `
vec3 openOcean(vec2 uv, float time) {
  float swell = sin(dot(uv, vec2(82.0, 55.0)) + time * 0.38)
    * sin(dot(uv, vec2(47.0, -69.0)) - time * 0.24);
  return vec3(0.023, 0.091, 0.109) + swell * vec3(0.003, 0.005, 0.006);
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
  float id = provinceAt(mapUv);
  vec2 lookup = lookupAt(id);
  float water = texture2D(waterLookup, lookup).r * 255.0;
  vec4 owner = texture2D(ownerLookup, lookup);
  vec4 historical = texture2D(historicalLookup, lookup);
  vec4 borderOwner = mapMode == 2 ? historical : owner;
  float sourceResolution = texture2D(precisionLookup, lookup).r * 255.0;
  float terrain = texture2D(terrainRaster, mapUv).r * 255.0;
  float river = texture2D(riverRaster, mapUv).r;

  vec2 pixel = mapUv * rasterSize;
  vec3 color;
  if (water > 0.5) {
    float depth = clamp((0.5 - texture2D(heightRaster, mapUv).r) * 7.0, 0.0, 1.0);
    color = mix(vec3(0.040, 0.145, 0.158), vec3(0.012, 0.071, 0.105), smoothstep(0.04, 0.72, depth));
    if (water > 1.5) color = mix(color, vec3(0.037, 0.130, 0.151), 0.5);
    float swell = sin(dot(mapUv, vec2(82.0, 55.0)) + seconds * 0.38)
      * sin(dot(mapUv, vec2(47.0, -69.0)) - seconds * 0.24);
    color += swell * vec3(0.003, 0.005, 0.006);
  } else {
    color = vec3(0.183, 0.276, 0.157);
    if (terrain > 1.5) color = vec3(0.296, 0.313, 0.175);
    if (terrain > 2.5) color = vec3(0.370, 0.365, 0.276);
    float light = dot(normalize(mapNormal), normalize(vec3(-0.48, 0.84, -0.25)));
    float altitude = clamp(mapHeight / 25.0, 0.0, 1.0);
    color = mix(color, vec3(0.60, 0.59, 0.48), altitude * 0.23);
    color *= 0.72 + 0.39 * clamp(light, 0.0, 1.0);
    color = mix(color, vec3(0.027, 0.136, 0.200), river * 0.7);
  }

  if (mapMode == 0) color = mix(color, owner.rgb, owner.a * 0.72);
  if (mapMode == 2) {
    color = mix(color, historical.rgb, historical.a * 0.74);
    if (sourceResolution > 1.5 && sourceResolution < 3.5) {
      float hatch = step(0.6, fract((pixel.x + pixel.y) * 0.12));
      color = mix(color, sourceResolution > 2.5 ? vec3(0.42, 0.46, 0.44) : vec3(0.11, 0.15, 0.17), hatch * 0.19);
    }
  }
  if (mapMode == 0 && hasOccupations) {
    vec4 occupier = texture2D(occupationLookup, lookup);
    float hatch = step(0.55, fract((pixel.x + pixel.y) * 0.20));
    color = mix(color, occupier.rgb, hatch * occupier.a * 0.76);
  }
  vec2 stepSize = clamp(fwidth(mapUv) * 0.62, 0.75 / rasterSize, 2.0 / rasterSize);
  float area = areaAt(id);
  vec3 border = max(
    max(boundariesAt(mapUv + vec2(stepSize.x, 0.0), id, area, borderOwner),
        boundariesAt(mapUv - vec2(stepSize.x, 0.0), id, area, borderOwner)),
    max(boundariesAt(mapUv + vec2(0.0, stepSize.y), id, area, borderOwner),
        boundariesAt(mapUv - vec2(0.0, stepSize.y), id, area, borderOwner))
  );
  vec2 wide = stepSize * 1.7;
  float country = max(border.z, max(
    max(boundariesAt(mapUv + vec2(wide.x, 0.0), id, area, borderOwner).z,
        boundariesAt(mapUv - vec2(wide.x, 0.0), id, area, borderOwner).z),
    max(boundariesAt(mapUv + vec2(0.0, wide.y), id, area, borderOwner).z,
        boundariesAt(mapUv - vec2(0.0, wide.y), id, area, borderOwner).z)
  ));
  float rasterPixelsPerScreenPixel = max(fwidth(mapUv.x) * rasterSize.x, fwidth(mapUv.y) * rasterSize.y);
  float detail = 1.0 - smoothstep(1.0, 3.5, rasterPixelsPerScreenPixel);
  float borderOpacity = border.x * (0.15 + 0.17 * detail) + border.y * 0.16 + country * 0.62;
  if (water > 0.5) borderOpacity *= 0.07;
  color = mix(color, water > 0.5 ? vec3(0.070, 0.153, 0.178) : vec3(0.066, 0.085, 0.057), clamp(borderOpacity, 0.0, 0.78));
  if (selectedId > 0.0 && abs(id - selectedId) < 0.5) {
    color = mix(color, vec3(0.84, 0.62, 0.27), 0.18 + border.x * 0.60);
  }

  float edgeDistance = min(min(mapUv.x, 1.0 - mapUv.x), min(mapUv.y, 1.0 - mapUv.y));
  float edgeFade = 1.0 - smoothstep(0.0, 0.045, edgeDistance);
  color = mix(color, openOcean(mapUv, seconds), edgeFade);

  gl_FragColor = vec4(color, 1.0);
  #include <colorspace_fragment>
}
`;
