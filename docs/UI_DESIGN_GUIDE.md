# UI_DESIGN_GUIDE (visual language, layout, components, map overlays)

Purpose: replace the current prototype look (dark teal boxes, native HTML controls, dev text, cluttered markers) with a coherent modern strategy-game UI.
Relation to other docs: behavior and interaction rules stay in VISUAL_AND_UI_SPEC.md; THIS file wins on visual design, layout, components and map overlays. The agent turns sections 3-5 into `docs/UI_STYLE.md` (tokens as code) in task UI-1.

DO NOT load the whole file. `grep -n "^## " docs/UI_DESIGN_GUIDE.md`, then `sed -n` the section you need.
Labels: MUST = acceptance requirement; SHOULD = default; UNVERIFIED = license/behavior not checked (check before use, log in ASSETS.md).

## Index
1. Audit of the current build (defects to eliminate)
2. Design direction and what we take from reference games
3. Design tokens (color, type, spacing, shape, motion)
4. Layout shell (wireframes)
5. Component specs
6. Icons, flags, imagery
7. Map overlays: unit markers, labels, tooltips, map modes
8. Window designs (province, country, event, diplomacy, war/peace)
9. Scaling, accessibility, performance
10. Implementation rules and tooling
11. Task breakdown (one PR each) and acceptance
12. Doc changes to apply

## 1. Audit of the current build (from the M5b-era screenshot)
U1 MUST FIX: unit markers render ON TOP of UI panels (crossed-swords icons visible over the right panel tabs, flag and stat tiles). Map overlays must sit below all UI and must not receive clicks through panels.
U2: one identical marker per province/army, no hierarchy, no clustering; markers cover country labels.
U3: country labels: all-caps bold with a star prefix, straight, same size regardless of country size, overlapping each other and markers in dense areas (Germany/Low Countries).
U4: developer content in the player UI: control hints paragraph, "WebGL smoke test" link, "Historical 1440" mode button.
U5: native HTML `<select>` for country and speed, plain outlined buttons, default focus styles.
U6: raw data key still shown ("Capital · london"); zero placeholders ("Monthly income 0", "Tribute paid 0") shown as if meaningful.
U7: six large text tabs in a 3x2 grid eat the panel; the context panel overflows the screen and scrolls with a default scrollbar.
U8: date/speed are plain text + select; no resource bar, no minimap, no alerts, map modes hidden inside an info box.
U9: flat pastel political colors with large same-hue blocks (very large pink areas); neighbors are not clearly distinct.
U10: generic look: teal translucent boxes, no hierarchy between title, numbers and labels.

## 2. Design direction
Chosen direction: "cartographer's war room": warm near-black surfaces, parchment-colored text, brass/gold accents, serif display for names and titles, clean sans for numbers and body, flat vector icons, thin ornament only (gold hairlines, corner brackets, ribbon headers). No faux-skeuomorphic textures that look cheap; we cannot paint frames/portraits, so quality comes from typography, spacing, iconography and consistency.
Take from reference games (conventions only; never copy their art, icons, frames or text):
- EU4: dense top bar with icon+number resources and rich tooltips; map modes and minimap grouped; notifications and alerts; right-click orders.
- Civ6: bold clear icons, color-coded banners on the map, large readable contextual panels, strong hierarchy.
- Age of Civilizations: minimal flat menus, large readable text, simple flows.
- CK3: portrait frames and rich tooltips for ruler/RP screens (later).
Legal: layout conventions are fine; do not trace, extract or copy assets from any commercial game. Reference screenshots may be used by the owner for personal comparison only and must not be committed.

## 3. Design tokens
Colors (warm dark; verify all text pairs >= 4.5:1 with a script, large text >= 3:1):
```css
:root{
  --bg-900:#12100d; --bg-800:#1a1713; --bg-700:#231f19; --bg-600:#2e2820;
  --line-500:#4a4132; --line-300:#6b5e47;
  --text-100:#f1e7d0; --text-300:#c9bb9a; --text-500:#8f8266;
  --gold-300:#e6c883; --gold-500:#c8a04a; --gold-700:#8a6b2a;
  --pos:#6fae5a; --neg:#e0675b; --warn:#e0a43a; --info:#5b9bc4;
  --shadow-panel:0 8px 24px rgba(0,0,0,.45);
}
```
Map owner palette: generated, not hand-picked. HSL with S 35-55%, L 45-60%, hue spread by graph coloring so adjacent countries differ by at least ~40 degrees of hue or lightness; big empires get the most distinct hues; player countries get fixed, highly distinct colors. No large pink/saturated blocks.
Typography (self-host woff2; the game must work offline on LAN; fonts UNVERIFIED but expected SIL OFL):
- Display/names: EB Garamond or Cormorant Garamond SemiBold (window titles 20-22 px, country names on map).
- UI/body/numbers: Source Sans 3 (or similar humanist sans) 14-16 px at 1080p; numbers with `font-variant-numeric: tabular-nums lining-nums`.
- Scale: 12 caption / 14 body / 16 emphasis / 20 title / 28 hero. Never below 12 px at UI scale 100%.
Spacing: 4 px grid (4, 8, 12, 16, 24, 32). Radius: 4 px controls, 6 px panels. Border 1 px `--line-500`; panel header underline 2 px `--gold-700`.
Surfaces: panels `--bg-800` at 94% opacity (no heavy backdrop blur over the WebGL canvas: expensive); raised rows `--bg-700`; hover `--bg-600`. Optional 4-6% noise overlay (procedural SVG or CC0 texture).
Motion: 120-180 ms ease-out for hover/press; 200 ms slide/fade for panels; number changes tick (brief count animation + green/red flash). Respect `prefers-reduced-motion`.
Elevation: panels use `--shadow-panel`; modals add a 55% dark scrim.

## 4. Layout shell (1920x1080 reference; must also work at 1280x720)
```
+----------------------------------------------------------------------------------+
| TOP BAR 56px: [Flag] Country v | Treasury (+/mo) | Manpower | ADM DIP MIL | ... | Date | Pause | Speed pips | Menu |
+---+------------------------------------------------------------+-----------------+
| L |                                                            | ALERTS / TOASTS |
| E |                                                            | stack, 320px    |
| F |              MAP FILLS THE WINDOW BEHIND ALL UI            +-----------------+
| T |                                                            | CONTEXT PANEL   |
|   |                                                            | selected        |
| R |                                                            | province/army/  |
| A |                                                            | country, 360px  |
| I |                                                            |                 |
| L |                                                            |                 |
|56 | +----------+   +------------------------+                  |                 |
|px | | MINIMAP  |   | SELECTION BAR (units)  |                  |                 |
|   | | + modes  |   | strength morale orders|                  |                 |
+---+-+----------+---+------------------------+------------------+-----------------+
```
- MUST: top bar shows only what exists in the sim today (country, treasury + monthly delta, ruler ADM/DIP/MIL, date, pause, speed). Slots for future resources are added with their milestones; no empty "0" placeholders.
- MUST: left rail = 56 px icon buttons (Country, Diplomacy, Military, Technology, Ideas, Reforms, Events/Log) with tooltips, hotkeys, notification badges; replaces the six text tabs. Windows open as panels/overlays from here.
- MUST: right context panel appears only on selection (province/army/country), max height = viewport minus top bar minus margins, internal scroll with a thin custom scrollbar, closable with Esc.
- MUST: minimap bottom-left with a viewport rectangle; under it an icon row of map modes (Political, Terrain, Diplomatic; Historical-1440 only in dev mode). Click minimap to move the camera.
- MUST: selection bar appears at the bottom center only when own units are selected.
- MUST: no permanent control-hint text; first launch shows a dismissible tip card; hotkeys live in Settings and in tooltips.
- MUST: developer items (WebGL smoke test, debug map modes, perf overlay) only behind a dev flag (`?debug` or a hotkey).
- Panels never overlap each other: a layout manager stacks/collapses them with defined priority (modal > context > alerts).

## 5. Component specs
All components live in `/ui/components`, are keyboard accessible, and are shown with every state in the dev-only gallery (section 10).
- Button: heights 32 (compact), 40 (default), 48 (modal primary). Variants: primary (gold fill, dark text), secondary (`--bg-700`, border), ghost, danger. States: hover, active (1 px press), focus (2 px `--gold-300` ring, 2 px offset), disabled (muted + tooltip reason, never silently dead).
- Icon button 40x40, tooltip with name + hotkey. Badge: 16 px circle top-right with count.
- Tabs: underline style in gold, icon + label; max 5 per panel; overflow into a menu.
- Dropdown/select: custom component (Radix-style primitives recommended, UNVERIFIED license; MIT expected). NO native `<select>`.
- Speed control: pause button + 5 pips (filled = active), click or hotkeys; shows PAUSED state clearly (pulsing amber label).
- Stat tile: icon + value (tabular) + delta (green/red with arrow) + label; whole tile has a tooltip with the modifier breakdown (source -> value). Zero is shown as "0" only when meaningful.
- Progress bars: 8 px, rounded, with end-cap value; semantic colors (strength gold, morale green->red, siege blue, supply amber).
- List row: 40-48 px, icon/flag left, primary + secondary text, right-aligned numbers, hover/selected states, virtualized when long.
- Tooltip: delay 300 ms (instant for icons), max width 360 px, structure: title (serif) / description / breakdown table / footer (hotkey or "why disabled"). Nested tooltips allowed one level. Never covers the hovered element.
- Panel frame: header 44 px (title, subtitle, close), body, optional footer with actions; corner brackets and gold underline as SVG; scrollbars 6 px thin.
- Modal: scrim 55%, centered, max 720 px wide, focus trapped, Esc behavior defined per modal type.
- Toast: 320 px, icon + text + optional action, 6 s default, click jumps to location, stack max 4, grouped duplicates.
- Banner/ribbon header for event and war windows (SVG shape).

## 6. Icons, flags, imagery
- One icon family with a consistent stroke: UI glyphs from a permissive set (Lucide ISC, Phosphor MIT, Tabler MIT: UNVERIFIED) plus thematic icons (swords, castle, ship, crown) from game-icons.net (CC BY 3.0 per author: attribution required; UNVERIFIED) or drawn by the agent as SVG on a 24 px grid. Custom resource icons (coin, manpower, ADM/DIP/MIL, siege, supply, morale) are drawn by us in the same style. Icons are SVG sprites colored by tokens.
- Flags: from the heraldry generator; shown as 3:2 rounded rectangles with a 1 px border and a subtle highlight; on the map as shield-shaped banners.
- Portraits/illustrations: placeholder silhouettes with owner color for now; keep frames reusable so real art can drop in later. No generated art without recording its source in ASSETS.md.
- Fonts and icons are bundled in the app; nothing loads from the internet at runtime.

## 7. Map overlays
Unit markers (replaces the current per-province swords):
- MUST: render markers in the scene or in a DOM layer strictly below all UI (z-order) with pointer events blocked under panels (fixes U1).
- MUST: marker = small shield banner in the owner color with unit-type icon and a stack count/strength number; one marker per province stack (merge armies in the same province); selected = gold ring; moving units show the path line + ETA.
- MUST visibility rules: own units always; enemies in countries at war or near own borders; neutral/foreign units hidden at low zoom (clustered to a single "N armies" badge when overlapping); size scales with zoom between min 20 px and max 44 px; ships use an anchor-style variant; no marker for empty provinces.
- SHOULD: allied units shown dimmer; fleets at sea slightly larger.
Labels (fixes U3):
- MUST: country label size from the country's on-screen extent; font EB Garamond SemiBold, letter-spaced caps for large countries, mixed case for small; ink color `#2b2216` with a 1-2 px parchment halo for contrast; no star prefix (capital gets a small star/dot marker on the map instead).
- MUST: collision handling: compute label boxes, rank by country size then importance, hide lower-ranked labels that overlap or do not fit; never overlap markers (markers win; labels shift or hide).
- MUST: LOD by zoom: far = country names only; medium = + large cities and sea names; near = + province/city names. Fade 150 ms.
- SHOULD: curved labels along the country's medial axis later (not required now). Sea names italic blue-gray, wide spacing.
Hover/selection: province hover card after 300 ms (name, owner flag, terrain, development, armies); selection outline + pulse (shader, see VISUAL_AND_UI_SPEC S3). Capital marker: small star. Ports: anchor icons shown at medium/near zoom or when relevant.
Map modes toolbar: icon buttons with tooltips (Political, Terrain, Diplomatic, later Trade/Religion). Dev-only: Historical 1440 comparison.
Palette (fixes U9): see section 3; the agent must regenerate country colors with the graph-coloring script and show before/after screenshots.

## 8. Window designs
Province panel (right context panel):
```
+------------------------------------+
| [Owner shield] London          [x] |
| Kingdom of England . Capital       |
+------------------------------------+
| [terrain] Plains   [dev] 12        |
| [Tax 3.0] [Production 2.5] [Manp 1.2]  <- stat tiles with tooltips
+------------------------------------+
| Armies here (2)               v    |
| Buildings                     v    |
+------------------------------------+
| [ Primary action ] [ Secondary ]   |
+------------------------------------+
```
Country window (from left rail): header with flag, ruler portrait placeholder; tabs Overview / Economy / Ruler & Court; economy shows income/expense breakdown as a stacked bar + table with tooltips; fiscal policy sliders show projected effect before Apply.
Event popup (M7): illustration banner 16:6, title (serif), body text <= ~70 characters per line at 16 px, options as large rows; each option shows effect chips (icon + signed value) BEFORE choosing; hovered option expands a tooltip with details; queued events shown as "1 of 3".
Diplomacy window (M6): 3 columns: country list (search + filters), country card (flag, ruler, opinion bar with factor tooltip), actions list; unavailable actions remain visible with the reason; proposals inbox/outbox; treaties tab with expiry.
War and peace window: header with sides and war score bar; participants list; occupied provinces list; peace builder (checkboxes/sliders for demands) with live acceptance bar and factor tooltip.
Selection bar (units): portrait/banner, name, commander, strength + morale bars, order buttons (Move, Stop, Split, Merge, Disband) with hotkeys, siege/occupation progress.

## 9. Scaling, accessibility, performance
- MUST: UI scale 80-150% via root `rem` variable; at 1280x720 and 1920x1080 and each scale there is no overflow, overlap or clipped text.
- MUST: keyboard navigation for all windows, visible focus ring, Esc closes top-most; tooltips reachable by focus.
- MUST: contrast checked by script (axe or equivalent, UNVERIFIED tool choice); color is never the only signal (icons/patterns/signs accompany red/green).
- SHOULD: colorblind-safe owner palette option; text-size option.
- MUST: UI does not cost frame time: no heavy backdrop-filter over the canvas, no layout thrash from per-tick updates (batch updates to animation frames), virtualized long lists.

## 10. Implementation rules and tooling
- Tailwind theme generated from the section 3 tokens (single source: tokens file -> CSS variables -> Tailwind). No hard-coded colors or px outside tokens.
- Component primitives for dropdown, tooltip, dialog, tabs from an accessible headless library (Radix UI or similar, UNVERIFIED) styled by our tokens.
- Dev-only `/ui-gallery` page rendering every component in every state at 100% and 150% scale; screenshots of it are part of UI-2 acceptance.
- All visible text via localization keys; linter fails on missing keys (U6).
- Screenshot command (from VISUAL_AND_UI_SPEC) captures: default view, province selected, army selected with path, event popup, diplomacy window, 1280x720 variant.
- Owner review is by side-by-side before/after screenshots in the PR description.

## 11. Task breakdown (each is one PR with screenshots at 1080p and 720p)
UI-1 Tokens, fonts, icon set, Tailwind theme, `docs/UI_STYLE.md`, gallery skeleton.
UI-2 Core components (button, icon button, dropdown, tabs, tooltip, panel, stat tile, bars, toast, modal) + gallery with all states.
UI-3 Layout shell: top bar, left rail, bottom areas, layout manager; remove dev text; custom speed control; fix U4-U8.
UI-4 Context panels: province, country, selection bar with real data and tooltip breakdowns; localized names (U6).
UI-5 Map overlays: marker z-order fix (U1), stack markers + visibility rules, label collision + LOD (U2, U3), hover card.
UI-6 Minimap + map-mode toolbar; owner palette regeneration (U9).
UI-7 Window shells: diplomacy, war/peace, event modal, log (visual shells with real data where it exists; behavior per M6/M7).
UI-8 Polish: motion, sounds hooks, UI scale/accessibility pass, final screenshot set.
Acceptance for the whole redesign (owner reviews screenshots):
- U1-U10 resolved; no native select; no dev text; no overlap between overlays and panels at 1280x720 and 1920x1080 at UI scales 80/100/150.
- Every number has a tooltip; every disabled control states why; contrast script passes.
- Consistent type scale and spacing; no hard-coded styles outside tokens.
- 60 FPS retained on the target GPU with the full UI; memory logged.
- Honest note in the PR if any element falls short of this guide.

## 12. Doc changes to apply (one docs PR; owner authorizes the single AGENTS.md row)
- AGENTS.md table: `| UI look, layout, components, map overlays | docs/UI_DESIGN_GUIDE.md |`.
- M05b_visual_pass.md: replace the old UI task 5b.6 with UI-1..UI-8 above (keep 5b.7/5b.8 behavior tasks from VISUAL_AND_UI_SPEC.md); acceptance adds "section 11 checklist".
- Conflict rule note in VISUAL_AND_UI_SPEC.md: visual design/layout/components defer to UI_DESIGN_GUIDE.md.
- ASSETS.md: register fonts and icon sets before use.
