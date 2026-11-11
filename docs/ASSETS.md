# ASSETS

## Rules
- No asset without a row below. Record: path, source URL, author, license, modifications.
- Allowed: CC0, public domain, CC-BY (with attribution recorded), own/generated work. Disallowed: unknown license, "free for personal use", ripped game assets.
- Check the license of EACH file, not just the pack.

## Pipeline
- 3D: Blender headless + `bpy` scripts in `/tools/models` -> export GLB. Units < 500 triangles, shared palette texture. Base packs: Kenney, Quaternius (CC0).
- Flags/coats of arms: parametric SVG generator in `/tools/heraldry`.
- UI icons: SVG, open-license icon sets. Textures: Krita/GIMP, power-of-two sizes, compressed where possible.
- Portraits: placeholders first; any generated art must be recorded here with the tool used.

## Register
| Path | Source | Author | License | Notes |
|------|--------|--------|---------|-------|
| `app/icons/icon.png` | Original geometric placeholder | Project team | Own work | 32×32 PNG generated for the M0 Tauri window |
| `app/icons/icon.ico` | Converted from `app/icons/icon.png` | Project team | Own work | Windows icon resource for Tauri |
