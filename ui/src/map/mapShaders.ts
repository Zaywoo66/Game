export const mapVertexShader = `
varying vec2 mapUv;
varying vec3 mapNormal;

void main() {
  mapUv = uv;
  mapNormal = normal;
  gl_Position = projectionMatrix * modelViewMatrix * vec4(position, 1.0);
}
`;

export const mapFragmentShader = `
uniform sampler2D provinceRaster;
uniform sampler2D terrainRaster;
uniform sampler2D riverRaster;
uniform sampler2D ownerLookup;
uniform sampler2D waterLookup;
uniform vec2 rasterSize;
uniform vec2 lookupSize;
uniform float selectedId;
uniform float seconds;

varying vec2 mapUv;
varying vec3 mapNormal;

float provinceAt(vec2 point) {
  vec3 rgb = floor(texture2D(provinceRaster, point).rgb * 255.0 + 0.5);
  return dot(rgb, vec3(65536.0, 256.0, 1.0));
}

vec2 lookupAt(float id) {
  return (vec2(mod(id, lookupSize.x), floor(id / lookupSize.x)) + 0.5) / lookupSize;
}

void main() {
  float id = provinceAt(mapUv);
  vec2 lookup = lookupAt(id);
  float water = texture2D(waterLookup, lookup).r * 255.0;
  vec4 owner = texture2D(ownerLookup, lookup);
  float terrain = texture2D(terrainRaster, mapUv).r * 255.0;
  float river = texture2D(riverRaster, mapUv).r;

  vec2 pixel = mapUv * rasterSize;
  vec3 color;
  if (water > 0.5) {
    float wave = sin(pixel.x * 0.12 + pixel.y * 0.095 - seconds * 1.35)
      * cos(pixel.y * 0.155 + seconds * 0.68);
    color = mix(vec3(0.020, 0.071, 0.105), vec3(0.038, 0.129, 0.160), smoothstep(0.0, 1.0, mapUv.y));
    if (water > 1.5) color = vec3(0.025, 0.111, 0.146);
    color += wave * vec3(0.006, 0.013, 0.017);
  } else {
    color = vec3(0.163, 0.262, 0.139);
    if (terrain > 1.5) color = vec3(0.252, 0.298, 0.157);
    if (terrain > 2.5) color = vec3(0.338, 0.337, 0.248);
    float light = dot(normalize(mapNormal), normalize(vec3(-0.50, 1.0, -0.25)));
    float textureGrain = sin(pixel.x * 0.53 + pixel.y * 0.31) * sin(pixel.y * 0.69);
    color *= 0.75 + 0.34 * clamp(light, 0.0, 1.0) + textureGrain * 0.025;
    color = mix(color, vec3(0.027, 0.136, 0.200), river * 0.7);
  }

  color = mix(color, owner.rgb, owner.a * 0.72);
  vec2 stepSize = clamp(fwidth(mapUv) * 0.65, 1.0 / rasterSize, 2.0 / rasterSize);
  float border = max(
    max(step(0.5, abs(id - provinceAt(mapUv + vec2(stepSize.x, 0.0)))),
      step(0.5, abs(id - provinceAt(mapUv - vec2(stepSize.x, 0.0))))),
    max(step(0.5, abs(id - provinceAt(mapUv + vec2(0.0, stepSize.y)))),
      step(0.5, abs(id - provinceAt(mapUv - vec2(0.0, stepSize.y)))))
  );
  color = mix(color, water > 0.5 ? vec3(0.084, 0.188, 0.226) : vec3(0.073, 0.098, 0.060), border * 0.56);
  if (selectedId > 0.0 && abs(id - selectedId) < 0.5) {
    color = mix(color, vec3(0.84, 0.62, 0.27), 0.18 + border * 0.60);
  }

  gl_FragColor = vec4(color, 1.0);
  #include <colorspace_fragment>
}
`;
