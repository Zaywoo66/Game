# UI style guide (M5b)

The current UI uses `ui/src/style.css` directly; there is no Tailwind config in
the repository. Shared colors, spacing, fonts and corner radius are CSS custom
properties on `:root` so panels and new controls can use the same tokens.

## Tokens

| Token | Value | Use |
|---|---|---|
| `--ui-color-ocean` | `#0d1a24` | map viewport background |
| `--ui-color-panel` | `#142630f2` | translucent overlay panels |
| `--ui-color-panel-solid` | `#12232d` | opaque dialog and dense panel backgrounds |
| `--ui-color-panel-raised` | `#203843` | inputs and raised controls |
| `--ui-color-border` | `#66838d99` | panel and control borders |
| `--ui-color-text` | `#edf2ef` | primary text |
| `--ui-color-muted` | `#b8c9ca` | secondary text |
| `--ui-color-accent` | `#d49b76` | copper selection and focus accent |
| `--ui-space-1..4` | `4, 8, 12, 16px` | compact spacing scale |
| `--ui-panel-radius` | `9px` | overlay panel corners |
| `--ui-font-body` | system UI sans serif | controls and body text |
| `--ui-font-heading` | Georgia serif | map panel headings |

## Layout and interaction

- The map fills the window. Panels overlay its edges; keep the center available
  for camera focus and unit movement.
- Keep the map mode and controls in a compact upper-left panel. Keep campaign
  time in the upper-right HUD and resource totals in a centered status strip.
- Use a scrollable right inspector below the time HUD for province, country,
  military and diplomacy details. The player can collapse it to clear the map
  and reopen it without losing the current selection.
- Use a deep coastal-blue surface, pale text, and restrained copper accents.
  A reference image can guide composition without dictating the game's palette.
- Keep WebGL smoke-test and source-attribution links in development/debug UI;
  they are not part of the normal campaign HUD.
- Unit badges use the owning country's color, a readable strength count, and a
  distinct selected outline. Army and fleet paths use separate dashed accents.
- Keyboard focus must remain visible. Overlay controls that accept input need a
  clear focus ring and accessible name.
- Keep game state changes host-driven: UI controls send commands and reflect
  only acknowledged snapshots or deltas.

## Current limits

This is the initial shared token set, not a full migration of existing CSS.
The interface still needs complete text localization, responsive review at
1280x720 and 1920x1080, and owner visual acceptance. The game shell now has a
compact HUD and collapsible inspector; new UI should use the CSS tokens by
default.
