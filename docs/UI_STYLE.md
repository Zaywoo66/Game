# UI style guide (M5b)

The current UI uses `ui/src/style.css` directly; there is no Tailwind config in
the repository. Shared colors, spacing, fonts and corner radius are CSS custom
properties on `:root` so panels and new controls can use the same tokens.

## Tokens

| Token | Value | Use |
|---|---|---|
| `--ui-color-ocean` | `#15232c` | map viewport background |
| `--ui-color-panel` | `#142329eb` | translucent overlay panels |
| `--ui-color-panel-solid` | `#142329` | opaque dialog and dense panel backgrounds |
| `--ui-color-panel-raised` | `#182e35` | inputs and raised controls |
| `--ui-color-border` | `#78918d80` | panel and control borders |
| `--ui-color-text` | `#f5ebd5` | primary text |
| `--ui-color-muted` | `#c5d2ca` | secondary text |
| `--ui-color-accent` | `#e1c48c` | selected states and focus outlines |
| `--ui-space-1..4` | `4, 8, 12, 16px` | compact spacing scale |
| `--ui-panel-radius` | `5px` | map-panel corners |
| `--ui-font-body` | system UI sans serif | controls and body text |
| `--ui-font-heading` | Georgia serif | map panel headings |

## Layout and interaction

- The map fills the window. Panels overlay its edges; keep the center available
  for camera focus and unit movement.
- Use compact, scrollable side panels for long country, military and diplomacy
  lists. Preserve the map below them instead of resizing the canvas.
- Unit badges use the owning country's color, a readable strength count, and a
  distinct selected outline. Army and fleet paths use separate dashed accents.
- Keyboard focus must remain visible. Overlay controls that accept input need a
  clear focus ring and accessible name.
- Keep game state changes host-driven: UI controls send commands and reflect
  only acknowledged snapshots or deltas.

## Current limits

This is the initial shared token set, not a full migration of existing CSS.
The interface still needs complete text localization, responsive review at
1280x720 and 1920x1080, and owner visual acceptance. The CSS tokens currently
replace the map shell and panel colors; new UI should use them by default.
