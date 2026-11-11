"""Generate the original infantry marker using Blender 4.5.3.

blender --background --factory-startup --python tools/models/build_infantry.py
"""

from pathlib import Path
from tempfile import TemporaryDirectory

import bpy


ROOT = Path(__file__).resolve().parents[2]
OUTPUT = ROOT / "ui" / "public" / "models" / "infantry.glb"
PALETTE = (
    ("boot", (52, 47, 45)),
    ("cloth", (157, 139, 111)),
    ("skin", (220, 169, 118)),
    ("metal", (185, 195, 197)),
    ("wood", (102, 78, 56)),
    ("shield", (169, 131, 80)),
)


def build():
    bpy.ops.wm.read_factory_settings(use_empty=True)
    source_materials = {
        name: bpy.data.materials.new(name) for name, _ in PALETTE
    }
    parts = []

    def finish(name, color):
        part = bpy.context.object
        part.name = name
        part.data.materials.append(source_materials[color])
        parts.append(part)

    def box(name, color, location, scale):
        bpy.ops.mesh.primitive_cube_add(size=1, location=location)
        bpy.context.object.scale = scale
        bpy.ops.object.transform_apply(location=False, rotation=False, scale=True)
        finish(name, color)

    def cylinder(name, color, location, radius, depth, sides=8):
        bpy.ops.mesh.primitive_cylinder_add(
            vertices=sides, radius=radius, depth=depth, location=location
        )
        finish(name, color)

    for index, x in enumerate((-0.13, 0.13)):
        box(f"Boot {index}", "boot", (x, -0.04, 0.08), (0.19, 0.32, 0.16))
        box(f"Leg {index}", "cloth", (x, 0, 0.30), (0.17, 0.23, 0.42))
    box("Tunic", "cloth", (0, 0, 0.73), (0.55, 0.31, 0.53))
    box("Belt", "boot", (0, 0, 0.60), (0.58, 0.34, 0.08))
    box("Shield arm", "cloth", (-0.33, -0.05, 0.79), (0.17, 0.25, 0.39))
    box("Spear arm", "cloth", (0.33, -0.05, 0.85), (0.17, 0.25, 0.29))
    cylinder("Head", "skin", (0, 0, 1.12), 0.14, 0.22)
    bpy.ops.mesh.primitive_cone_add(
        vertices=8, radius1=0.18, radius2=0.075, depth=0.20, location=(0, 0, 1.29)
    )
    finish("Helmet", "metal")
    cylinder("Helmet rim", "metal", (0, 0, 1.20), 0.19, 0.055)

    bpy.ops.mesh.primitive_cylinder_add(
        vertices=8, radius=0.25, depth=0.075, rotation=(1.57079632679, 0, 0),
        location=(-0.35, -0.25, 0.70),
    )
    finish("Shield", "shield")
    box("Shield boss", "metal", (-0.35, -0.296, 0.70), (0.115, 0.055, 0.115))
    cylinder("Spear shaft", "wood", (0.41, -0.13, 0.90), 0.025, 1.71, sides=6)
    bpy.ops.mesh.primitive_cone_add(
        vertices=4, radius1=0.061, depth=0.20, location=(0.41, -0.13, 1.84)
    )
    finish("Spear tip", "metal")

    bpy.ops.object.select_all(action="SELECT")
    bpy.context.view_layer.objects.active = parts[0]
    bpy.ops.object.join()
    model = bpy.context.object
    model.name = "Infantry"
    bpy.ops.object.transform_apply(location=True, rotation=True, scale=True)

    mesh = model.data
    palette_indices = {name: index for index, (name, _) in enumerate(PALETTE)}
    uv = mesh.uv_layers.active or mesh.uv_layers.new(name="Palette")
    for polygon in mesh.polygons:
        source = mesh.materials[polygon.material_index].name
        center = (palette_indices[source] + 0.5) / len(PALETTE)
        for index in polygon.loop_indices:
            uv.data[index].uv = (center, 0.5)
        polygon.material_index = 0

    material = bpy.data.materials.new("Infantry palette")
    material.use_nodes = True
    mesh.materials.clear()
    mesh.materials.append(material)
    mesh.calc_loop_triangles()
    triangle_count = len(mesh.loop_triangles)
    if triangle_count >= 500:
        raise RuntimeError(f"Infantry exceeds triangle budget: {triangle_count}")

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    with TemporaryDirectory() as folder:
        image = bpy.data.images.new("Infantry palette", width=len(PALETTE), height=1, alpha=False)
        image.pixels = [
            channel
            for _, rgb in PALETTE
            for channel in (*[value / 255 for value in rgb], 1.0)
        ]
        image.filepath_raw = str(Path(folder) / "palette.png")
        image.file_format = "PNG"
        image.save()
        image.pack()
        shader = material.node_tree.nodes.get("Principled BSDF")
        shader.inputs["Roughness"].default_value = 0.87
        texture = material.node_tree.nodes.new("ShaderNodeTexImage")
        texture.image = image
        texture.interpolation = "Closest"
        material.node_tree.links.new(texture.outputs["Color"], shader.inputs["Base Color"])
        bpy.ops.export_scene.gltf(filepath=str(OUTPUT), export_format="GLB", export_yup=True)

    print(f"Generated {OUTPUT.relative_to(ROOT)} with {triangle_count} triangles (Blender {bpy.app.version_string})")


if __name__ == "__main__":
    build()
