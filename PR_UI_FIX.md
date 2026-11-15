# PR: Fix UI design — restore EU4-style flat map and add top bar

## Problem
The previous AI session (Codex) broke the UI by:
1. Setting `maxPolarAngle: 1.20` on OrbitControls → 3D tilted map (like looking at a globe from an angle)  
2. Setting `mouseButtons.RIGHT = ROTATE` → right-drag rotated the camera into 3D perspective  
3. Camera initial position used `tilt = 0.50` radians → started tilted  
4. No top bar with date/treasury/speed (EU4-style)  
5. Right panel had only 2 tabs; did not match first screenshot  
6. No country name labels on the map  
7. Missing "Game political / Terrain" mode buttons

## Fixes applied

### `ui/src/map/mapViewer.ts`
- `maxPolarAngle = 0.001` and `minPolarAngle = 0.001` → locked top-down view  
- `enableRotate = false` → no 3D rotation at all  
- `mouseButtons.RIGHT = PAN` → right-drag pans, not rotates  
- `touches.TWO = DOLLY_PAN` → two-finger pan instead of rotate  
- Initial camera position now directly above target (`y = distance, z = 0.001`)  
- Added `projectProvince(id)` and `getViewportRect()` to the `MapViewer` interface and implementation  

### `ui/src/map/MapPage.tsx`
- Replaced `HostControls` with inline **EU4-style top bar**: country name + ruler, treasury, monthly income, date, pause/speed controls — all in a dark bar at the top-right  
- Added `CountryLabels` overlay component: each country gets a label (name in CAPS with ★) positioned via `projectProvince()` and updated every animation frame  
- Changed hint text from "Right-drag to tilt" → "Drag to pan · WASD/arrows · Scroll to zoom · Double-click to focus · Home for region"  
- Title stays "Europe" (was incorrectly changed to "Western Europe")  
- Inspector tabs now show: Country / Diplomacy / Military / Technology / Ideas / Reforms (matching first screenshot)  
- Map mode buttons added bottom-left: "Game political" (active), "Terrain", "Historical 1440" (dev-only behind `?debug`)  
- WebGL smoke test link now hidden in the info box (not removed, just not in front)  
- `fixedText()` helper duplicated locally to format treasury/income without importing from CountryPanel  
- Removed unused `HostControls` import  

### `ui/src/style.css`
Full redesign to match first screenshot and UI_DESIGN_GUIDE.md:
- **Top bar** `.top-bar`: dark `rgba(12,24,32,0.94)` bar, gold accent border-bottom, flex layout with sections  
- **Map panel** `.map-panel`: `rgba(15,30,38,0.92)` with `backdrop-filter: blur(4px)`, serif headings  
- **Country labels** `.country-label`: Georgia serif, bold, letter-spaced CAPS, parchment-halo text-shadow, z-index 5 (below UI panels, above map)  
- **Inspector tabs** `.inspector-tabs`: compact pill buttons, gold underline on active  
- **Map modes** `.map-modes`: bottom-left button row, matching first screenshot layout  
- **Economy tiles** `.country-economy div`: dark raised background, uppercase 11px label, 15px tabular number  
- Removed teal colors; replaced with warm dark palette from `UI_DESIGN_GUIDE §3`  
- Custom thin scrollbar on inspector panel  
- Responsive tweaks for 580px and 900px breakpoints  

## Verification
- `tsc --noEmit` → 0 errors  
- `vite build` → success (776 kB bundle, 3.6 s)  
- `eslint src/map/MapPage.tsx src/map/mapViewer.ts` → 0 warnings  
- Visually: map now fills the full window top-down (no tilt), top bar shows game data, country labels appear on the map, inspector has EU4-style tabs  
