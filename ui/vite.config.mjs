/* eslint-env node */
import { readFileSync } from 'node:fs';
import { defineConfig } from 'vite';

const regions = ['western_europe', 'europe'];
const files = new Map([
  ['provinces.json', 'application/json'],
  ['provinces.png', 'image/png'],
  ['heightmap.png', 'image/png'],
  ['terrain.png', 'image/png'],
  ['rivers.png', 'image/png'],
  ['names.en.json', 'application/json'],
  ['polities_1440.json', 'application/json'],
]);

function readMapAsset(region, file) {
  return readFileSync(new URL(`../data/map/${region}/${file}`, import.meta.url));
}

function mapAssets() {
  return {
    name: 'strategy-map-assets',
    configureServer(server) {
      server.middlewares.use((request, response, next) => {
        const pathname = request.url?.split('?')[0] ?? '';
        const region = regions.find((item) => pathname.startsWith(`${server.config.base}map/${item}/`));
        const file = region ? pathname.slice(`${server.config.base}map/${region}/`.length) : '';
        const type = files.get(file);
        if (!type || (request.method !== 'GET' && request.method !== 'HEAD')) return next();
        try {
          const data = readMapAsset(region, file);
          response.setHeader('Content-Type', type);
          response.setHeader('Content-Length', data.length);
          response.end(request.method === 'HEAD' ? undefined : data);
        } catch (error) {
          next(error);
        }
      });
    },
    generateBundle() {
      for (const region of regions) for (const file of files.keys()) {
        this.emitFile({ type: 'asset', fileName: `map/${region}/${file}`, source: readMapAsset(region, file) });
      }
    },
  };
}

function eventLocaleAsset() {
  const fileName = 'locales/events.en.json';
  const readLocale = () => readFileSync(new URL('../data/locales/events.en.json', import.meta.url));
  return {
    name: 'strategy-event-locale',
    configureServer(server) {
      const route = `${server.config.base}${fileName}`;
      server.middlewares.use((request, response, next) => {
        const pathname = request.url?.split('?')[0] ?? '';
        if (pathname !== route || (request.method !== 'GET' && request.method !== 'HEAD')) {
          return next();
        }
        try {
          const data = readLocale();
          response.setHeader('Content-Type', 'application/json');
          response.setHeader('Content-Length', data.length);
          response.end(request.method === 'HEAD' ? undefined : data);
        } catch (error) {
          next(error);
        }
      });
    },
    generateBundle() {
      this.emitFile({ type: 'asset', fileName, source: readLocale() });
    },
  };
}

export default defineConfig({
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  plugins: [mapAssets(), eventLocaleAsset()],
});
