"""Deterministic miniature Chinese architecture; run only in a fresh Blender process.

/Applications/Blender.app/Contents/MacOS/Blender --background --factory-startup \
    --python tools/build_riverside_assets.py -- --output assets/models/riverside

All modeling helpers use engine (X, up-Y, Z) coordinates. Blender data is Z-up.
The small uncompressed GLB writer reads Blender's evaluated triangle/corner data
and bakes the inverse coordinate conversion into positions AND normals. There
are no export operator defaults, node rotations, textures, or material splits.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path
import struct
import sys

import bpy
from mathutils import Vector

GENERATOR_VERSION = "1.0.0"
DIMENSIONS = {
    "datiji": (7, 2, 7),
    "hongzhu4": (1, 4, 1),
    "liangfang5": (5, 1, 1),
    "louban5": (5, 1, 5),
    "dougong": (2, 2, 2),
    "jiangting_roof": (7, 3, 7),
}
# Linear colors: rendered unchanged with a white Bevy StandardMaterial.
COLORS = {
    "stone": (0.39, 0.44, 0.43, 1),
    "stone_light": (0.57, 0.61, 0.56, 1),
    "stone_dark": (0.19, 0.24, 0.23, 1),
    "vermilion": (0.48, 0.037, 0.019, 1),
    "red_light": (0.66, 0.073, 0.025, 1),
    "wood": (0.105, 0.031, 0.018, 1),
    "wood_light": (0.22, 0.074, 0.031, 1),
    "gold": (0.83, 0.49, 0.095, 1),
    "gold_dark": (0.46, 0.24, 0.047, 1),
    "jade": (0.026, 0.25, 0.19, 1),
    "teal": (0.021, 0.17, 0.15, 1),
    "jade_light": (0.058, 0.35, 0.25, 1),
}


class Builder:
    """Disconnected decorative solids intentionally share one direct data mesh."""

    def __init__(self):
        self.vertices = []
        self.faces = []
        self.colors = []

    def part(self, vertices, faces, color):
        offset = len(self.vertices)
        self.vertices.extend(vertices)
        for face in faces:
            # Hipped roof apex rows can collapse: never emit zero-area faces.
            points = [Vector(vertices[i]) for i in face]
            if sum((points[i] - points[0]).cross(points[i + 1] - points[0]).length
                   for i in range(1, len(points) - 1)) < 1e-9:
                continue
            self.faces.append(tuple(offset + i for i in face))
            self.colors.append(COLORS[color])

    def box(self, center, size, color):
        x, y, z = center
        a, b, c = (v / 2 for v in size)
        self.part([(x-a, y-b, z-c), (x+a, y-b, z-c),
                   (x+a, y+b, z-c), (x-a, y+b, z-c),
                   (x-a, y-b, z+c), (x+a, y-b, z+c),
                   (x+a, y+b, z+c), (x-a, y+b, z+c)],
                  [(0, 3, 2, 1), (4, 5, 6, 7), (0, 1, 5, 4),
                   (3, 7, 6, 2), (0, 4, 7, 3), (1, 2, 6, 5)], color)

    def tube(self, points, radius, color, sides=8):
        points = [Vector(p) for p in points]
        vertices = []
        u = None
        for i, point in enumerate(points):
            tangent = (points[min(i + 1, len(points)-1)] - points[max(i-1, 0)]).normalized()
            if u is None:
                reference = Vector((0, 1, 0)) if abs(tangent.y) < 0.9 else Vector((1, 0, 0))
                u = tangent.cross(reference).normalized()
            else:
                # Transport the previous ring frame: choosing a new world-axis
                # reference at every bend introduces abrupt, inverted tube faces.
                u = (u - tangent * u.dot(tangent)).normalized()
            v = tangent.cross(u).normalized()
            for j in range(sides):
                angle = 2 * math.pi * j / sides
                vertices.append(tuple(point + radius * (u * math.cos(angle) + v * math.sin(angle))))
        faces = [tuple(reversed(range(sides))),
                 tuple((len(points)-1)*sides+j for j in range(sides))]
        faces.extend((i*sides+j, i*sides+(j+1)%sides,
                      (i+1)*sides+(j+1)%sides, (i+1)*sides+j)
                     for i in range(len(points)-1) for j in range(sides))
        self.part(vertices, faces, color)

    def column(self, center, radius, height, color, sides=16):
        x, y, z = center
        self.tube([(x, y-height/2, z), (x, y+height/2, z)], radius, color, sides)

    def finish(self, name, dimensions, material):
        lo = [min(v[i] for v in self.vertices) for i in range(3)]
        hi = [max(v[i] for v in self.vertices) for i in range(3)]
        fitted = [tuple((v[i]-(lo[i]+hi[i])/2)*dimensions[i]/(hi[i]-lo[i])
                        for i in range(3)) for v in self.vertices]
        mesh = bpy.data.meshes.new(name)
        mesh.from_pydata([(x, -z, y) for x, y, z in fitted], [], self.faces)
        mesh.update()
        color = mesh.color_attributes.new(name="COLOR_0", type="FLOAT_COLOR", domain="CORNER")
        for polygon, rgba in zip(mesh.polygons, self.colors):
            for index in polygon.loop_indices:
                color.data[index].color = rgba
        mesh.color_attributes.active_color = color
        mesh.materials.append(material)
        obj = bpy.data.objects.new(name, mesh)
        bpy.context.scene.collection.objects.link(obj)
        obj["engine_dimensions"] = list(dimensions)
        obj["coordinate_contract"] = "Baked Y-up GLB; centered Mesh0/Primitive0; COLOR_0"
        return obj


def platform(b):
    for y, height, width, color in [(-0.86, .28, 7, "stone_dark"),
                                   (-.59, .26, 6.8, "stone_light"),
                                   (-.32, .28, 6.46, "stone"),
                                   (.16, .68, 6.04, "stone"),
                                   (.56, .12, 6.25, "stone_dark"),
                                   (.74, .24, 6.44, "stone_light"),
                                   (.93, .14, 6.28, "stone")]:
        b.box((0, y, 0), (width, height, width), color)
    # Recess-like stone panels, relief corner piers and cloud-scroll carvings.
    for side in range(4):
        def orient(x, y, z):
            for _ in range(side):
                x, z = -z, x
            return x, y, z
        for x in [-2.5, -1.25, 0, 1.25, 2.5]:
            size = (1.04, .39, .025) if side % 2 == 0 else (.025, .39, 1.04)
            b.box(orient(x, .13, 3.032), size, "stone_dark")
            for direction in [-1, 1]:
                points = []
                for i in range(13):
                    angle = i * math.pi * 1.7 / 12
                    radius = .145 * (1-i/17)
                    points.append(orient(x + direction*(.12 + radius*math.cos(angle)),
                                         .13 + radius*math.sin(angle), 3.052))
                b.tube(points, .023, "stone_light", 5)
        for x in [-2.92, 2.92]:
            b.box(orient(x, .12, 3.04), (.15, .62, .15), "stone_light")
    # Raised individual paving slabs leave readable grout without textures.
    for x in range(7):
        for z in range(7):
            b.box(((x-3)*.87, 1.009, (z-3)*.87), (.848, .025, .848),
                  "stone_light" if (x+2*z)%4 == 0 else "stone")


def column(b):
    b.box((0, -1.85, 0), (1, .3, 1), "stone_dark")
    for y, r, h, color in [(-1.62, .47, .2, "stone_light"),
                            (-1.44, .4, .16, "gold_dark"),
                            (-1.31, .37, .11, "gold"),
                            (.12, .33, 2.78, "vermilion"),
                            (1.49, .36, .12, "gold"),
                            (1.65, .42, .20, "vermilion"),
                            (1.82, .46, .14, "gold_dark")]:
        b.column((0, y, 0), r, h, color, 20)
    b.box((0, 1.945, 0), (.92, .11, .92), "wood")
    for i in range(8):
        a = i*math.tau/8
        x, z = .333*math.cos(a), .333*math.sin(a)
        b.tube([(x, -1.16, z), (x, -1.01, z)], .016, "gold", 5)


def beam(b):
    b.box((0, 0, 0), (5, .82, .78), "wood")
    for y in [-.455, .455]:
        b.box((0, y, 0), (5, .09, 1), "vermilion")
    for sign in [-1, 1]:
        b.box((0, 0, sign*.402), (4.66, .49, .04), "vermilion")
        for y in [-.30, .30]:
            b.box((0, y, sign*.445), (4.82, .045, .05), "gold")
        for x in [-2.15, -1.55, 0, 1.55, 2.15]:
            # Raised diamond medallions on both visible beam faces.
            b.part([(x-.18, 0, sign*.455), (x, -.18, sign*.455),
                    (x+.18, 0, sign*.455), (x, .18, sign*.455)],
                   [(0, 1, 2, 3) if sign > 0 else (3, 2, 1, 0)], "gold")
        for x in [-.95, .95]:
            b.tube([(x-.35, -.05, sign*.45), (x-.17, .10, sign*.45),
                    (x, -.07, sign*.45), (x+.17, .10, sign*.45),
                    (x+.35, -.05, sign*.45)], .024, "gold_dark", 6)
    for x in [-2.38, 2.38]:
        b.box((x, 0, 0), (.10, .87, .91), "gold_dark")


def floor(b):
    b.box((0, -.15, 0), (5, .70, 5), "wood")
    for x in [-2.39, 2.39]:
        b.box((x, .33, 0), (.22, .34, 5), "vermilion")
    for z in [-2.39, 2.39]:
        b.box((0, .33, z), (4.56, .34, .22), "vermilion")
    for i in range(12):
        b.box(((i-5.5)*.375, .33, 0), (.357, .30, 4.54),
              "wood_light" if i%3 else "wood")
    for sign in [-1, 1]:
        for y in [-.32, .12]:
            b.box((0, y, sign*2.505), (4.92, .035, .025), "gold_dark")
            b.box((sign*2.505, y, 0), (.025, .035, 4.92), "gold_dark")


def bracket(b):
    b.box((0, -.85, 0), (.7, .3, .7), "wood")
    b.box((0, -.61, 0), (.85, .18, .85), "gold_dark")
    # Three stepped interlocking arms with rising ends, not a stack of cubes.
    for tier, length in enumerate([1.16, 1.58, 2.0]):
        y = -.42 + tier*.46
        for axis in [0, 2]:
            profile = [(-length/2, y+.23), (-length/2, y+.03),
                       (-length*.27, y-.14), (length*.27, y-.14),
                       (length/2, y+.03), (length/2, y+.23),
                       (length*.30, y+.12), (-length*.30, y+.12)]
            verts = [(x, h, z) if axis == 0 else (z, h, -x)
                     for z in [-.16, .16] for x, h in profile]
            n = len(profile)
            faces = [tuple(reversed(range(n))), tuple(range(n, 2*n))]
            faces.extend((i, (i+1)%n, (i+1)%n+n, i+n) for i in range(n))
            b.part(verts, faces, "vermilion" if tier != 1 else "wood_light")
            for sign in [-1, 1]:
                p = sign*(length/2-.14)
                center = (p, y+.28, 0) if axis == 0 else (0, y+.28, -p)
                b.box(center, (.28, .14, .28), "gold")
    b.box((0, .90, 0), (1.24, .2, 1.24), "wood")


def roof_point(side, t, v):
    y = 1.05 - 2.48*v + .40*v**4 + .62*abs(t)**5*v**4
    if side < 2:
        return (t*(1.10+2.15*v), y, (1 if side == 0 else -1)*3.25*v)
    return ((1 if side == 2 else -1)*(1.10+2.15*v), y, t*3.25*v)


def roof(b):
    # Four swept hip surfaces meet a horizontal ridge. Curvature lifts the
    # corners above the eave centers; the short hips close the ridge ends.
    for side in range(4):
        columns, rows = (20 if side < 2 else 14), 8
        for i in range(columns):
            for j in range(rows):
                t0, t1 = -1+2*i/columns, -1+2*(i+1)/columns
                v0, v1 = j/rows, (j+1)/rows
                points = [roof_point(side, t0, v0), roof_point(side, t1, v0),
                          roof_point(side, t1, v1), roof_point(side, t0, v1)]
                # Each quad is planar enough for triangulation, orient outward/up.
                normal_y = sum((Vector(points[k])-Vector(points[0])).cross(
                    Vector(points[k+1])-Vector(points[0])).y for k in [1, 2])
                if normal_y < 0:
                    points.reverse()
                b.part(points, [(0, 1, 2), (0, 2, 3)],
                       ["jade", "teal", "jade_light"][(i+2*j)%3])
        # Hip boundaries already carry gold ridges. Emitting tile ribs from
        # both adjacent panels here creates duplicate triangles on four seams.
        for i in range(1, columns):
            t = -1 + 2*i/columns
            points = [tuple(c+(0.031 if axis == 1 else 0)
                            for axis, c in enumerate(roof_point(side, t, j/rows)))
                      for j in range(rows+1)]
            b.tube(points, .029, "jade_light", 5)
        eave = [roof_point(side, -1+2*i/columns, 1) for i in range(columns+1)]
        b.tube(eave, .075, "gold_dark", 6)
        b.tube([(x, y-.11, z) for x, y, z in eave], .07, "vermilion", 6)
        # Closed fascia: a darker second skin beneath the glazed top.
        for i in range(columns):
            p, q = eave[i:i+2]
            b.part([p, q, (q[0], q[1]-.19, q[2]), (p[0], p[1]-.19, p[2])],
                   [(0, 1, 2, 3) if side in [0, 3] else (3, 2, 1, 0)], "wood")
    # Solid dark soffit and red/gold concentric support bands.
    for y, height, width, color in [(-1.20, .22, 5.22, "wood"),
                                   (-1.36, .10, 4.84, "vermilion"),
                                   (-1.44, .06, 4.64, "gold_dark")]:
        b.box((0, y, 0), (width, height, width), color)
    for side in [0, 1]:
        for t in [-1, 1]:
            points = [roof_point(side, t, j/10) for j in range(11)]
            b.tube([(x, y+.072, z) for x, y, z in points], .077, "gold", 7)
            x, y, z = points[-1]
            b.tube([(x*.94, y+.07, z*.94), (x, y+.18, z),
                    (x*1.03, y+.36, z*1.03)], .070, "gold", 7)
    b.tube([(-1.24, 1.12, 0), (0, 1.12, 0), (1.24, 1.12, 0)], .13, "gold", 10)
    for sign in [-1, 1]:
        b.tube([(sign*.97, 1.12, 0), (sign*1.26, 1.23, 0),
                (sign*1.39, 1.43, 0), (sign*1.32, 1.51, 0)], .085, "gold", 8)
    b.column((0, 1.32, 0), .16, .30, "gold", 8)


def export_glb(obj, path):
    """Serialize actual Blender triangles, not procedural input estimates."""
    mesh = obj.data
    mesh.calc_loop_triangles()
    colors = mesh.color_attributes["COLOR_0"]
    positions, normals, rgba, indices = [], [], [], []
    dedup = {}
    for triangle in mesh.loop_triangles:
        for loop_index in triangle.loops:
            co = mesh.vertices[mesh.loops[loop_index].vertex_index].co
            n = triangle.normal
            key = tuple(co) + tuple(n) + tuple(colors.data[loop_index].color)
            if key not in dedup:
                dedup[key] = len(positions)
                positions.append((co.x, co.z, -co.y))
                normals.append((n.x, n.z, -n.y))
                rgba.append(tuple(colors.data[loop_index].color))
            indices.append(dedup[key])
    bounds = {"min": [min(v[i] for v in positions) for i in range(3)],
              "max": [max(v[i] for v in positions) for i in range(3)]}
    blob = bytearray()
    views, accessors = [], []
    for values, fmt, kind, component, target in [
        (positions, "3f", "VEC3", 5126, 34962),
        (normals, "3f", "VEC3", 5126, 34962),
        (rgba, "4f", "VEC4", 5126, 34962),
        ([(i,) for i in indices], "I", "SCALAR", 5125, 34963),
    ]:
        start = len(blob)
        for value in values:
            blob.extend(struct.pack("<"+fmt, *value))
        views.append({"buffer": 0, "byteOffset": start, "byteLength": len(blob)-start, "target": target})
        accessors.append({"bufferView": len(views)-1, "componentType": component,
                          "count": len(values), "type": kind})
    accessors[0].update(bounds)
    gltf = {
        "asset": {"version": "2.0", "generator": f"riverside {GENERATOR_VERSION}; Blender {bpy.app.version_string}"},
        "scene": 0, "scenes": [{"nodes": [0]}],
        "nodes": [{"name": obj.name, "mesh": 0}],
        "meshes": [{"name": obj.name, "primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1, "COLOR_0": 2},
                                                      "indices": 3, "mode": 4, "material": 0}]}],
        "materials": [{"name": "WhiteVertexColor", "pbrMetallicRoughness": {
            "baseColorFactor": [1, 1, 1, 1], "metallicFactor": 0, "roughnessFactor": .78}}],
        "accessors": accessors, "bufferViews": views, "buffers": [{"byteLength": len(blob)}],
    }
    document = json.dumps(gltf, sort_keys=True, separators=(",", ":")).encode()
    document += b" " * (-len(document) % 4)
    output = struct.pack("<III", 0x46546C67, 2, 28+len(document)+len(blob))
    output += struct.pack("<I4s", len(document), b"JSON") + document
    output += struct.pack("<I4s", len(blob), b"BIN\0") + blob
    path.write_bytes(output)
    return {"file": path.name, "dimensions": list(DIMENSIONS[obj.name]),
            "bounds": bounds, "vertices": len(positions), "triangles": len(indices)//3,
            "meshes": 1, "primitives": 1, "color_attribute": "COLOR_0",
            "colors_linear_rgba": [list(c) for c in sorted(set(rgba))],
            "bytes": len(output), "sha256": hashlib.sha256(output).hexdigest()}


def preview(objects, destination):
    """Temporary assembled pavilion scene, never saved into the source blend."""
    scene = bpy.context.scene
    for obj in objects.values():
        obj.hide_render = True
    def place(name, location, angle=0):
        obj = bpy.data.objects.new("preview_"+name, objects[name].data)
        scene.collection.objects.link(obj)
        x, y, z = location
        obj.location = (x, -z, y)
        obj.rotation_euler.z = angle
    place("datiji", (0, 0, 0))
    for x in [-2, 2]:
        for z in [-2, 2]:
            place("hongzhu4", (x, 3, z))
            place("dougong", (x, 5.5, z))
    for z in [-2, 2]:
        place("liangfang5", (0, 5, z))
    for x in [-2, 2]:
        place("liangfang5", (x, 5, 0), math.pi/2)
    place("jiangting_roof", (0, 8, 0))
    camera_data = bpy.data.cameras.new("preview_camera")
    camera = bpy.data.objects.new("preview_camera", camera_data)
    scene.collection.objects.link(camera)
    camera.location = (13, -17, 12)
    camera.rotation_euler = (Vector((0, 0, 4.1))-camera.location).to_track_quat('-Z', 'Y').to_euler()
    camera_data.type = 'ORTHO'
    camera_data.ortho_scale = 14
    scene.camera = camera
    for name, location, energy, size in [("key", (3, -8, 14), 1900, 9),
                                          ("fill", (-8, -1, 9), 1300, 8),
                                          ("rim", (2, 8, 13), 2200, 7)]:
        data = bpy.data.lights.new(name, 'AREA')
        data.energy, data.shape, data.size = energy, 'DISK', size
        light = bpy.data.objects.new(name, data)
        scene.collection.objects.link(light)
        light.location = location
        light.rotation_euler = (Vector((0, 0, 4))-light.location).to_track_quat('-Z', 'Y').to_euler()
    scene.world.color = (.24, .24, .24)
    scene.render.engine = 'CYCLES'
    scene.cycles.samples = 24
    scene.render.resolution_x = 1000
    scene.render.resolution_y = 1000
    scene.render.resolution_percentage = 100
    scene.render.filepath = str(destination)
    bpy.ops.render.render(write_still=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=Path, default=Path(__file__).resolve().parents[1]/"assets/models/riverside")
    parser.add_argument("--preview", type=Path)
    args = parser.parse_args(sys.argv[sys.argv.index("--")+1:] if "--" in sys.argv else [])
    if bpy.data.filepath:
        raise RuntimeError("Run --background --factory-startup without a user .blend; source scenes are never cleared")
    bpy.context.preferences.filepaths.save_version = 0
    for obj in list(bpy.data.objects):
        bpy.data.objects.remove(obj, do_unlink=True)
    material = bpy.data.materials.new("WhiteVertexColor")
    if not material.use_nodes:
        material.use_nodes = True
    shader = material.node_tree.nodes.get("Principled BSDF")
    shader.inputs["Roughness"].default_value = .78
    attribute = material.node_tree.nodes.new("ShaderNodeVertexColor")
    attribute.layer_name = "COLOR_0"
    material.node_tree.links.new(attribute.outputs["Color"], shader.inputs["Base Color"])
    args.output.mkdir(parents=True, exist_ok=True)
    builders = dict(zip(DIMENSIONS, [platform, column, beam, floor, bracket, roof]))
    objects, records = {}, []
    for name, dimensions in DIMENSIONS.items():
        builder = Builder()
        builders[name](builder)
        obj = builder.finish(name, dimensions, material)
        objects[name] = obj
        record = export_glb(obj, args.output/(name+".glb"))
        records.append(record)
        print(f"EXPORTED {name}: {record['triangles']} triangles, {record['bytes']} bytes, bounds={record['bounds']}")
    # Independent readback validates the bytes on disk BEFORE publishing manifest.
    sys.path.insert(0, str(Path(__file__).resolve().parent))
    from test_riverside_assets import validate_asset
    for record in records:
        validate_asset(args.output/record["file"], record["dimensions"])
    if sum(r["bytes"] for r in records) >= 3_000_000:
        raise RuntimeError("Combined GLB budget exceeded")
    manifest = {"schema_version": 1, "generator": "tools/build_riverside_assets.py",
                "generator_version": GENERATOR_VERSION,
                "generator_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
                "blender_version": bpy.app.version_string,
                "blender_build_hash": bpy.app.build_hash.decode(),
                "coordinates": "glTF Y-up meters; centered origin; baked vertices and normals; identity nodes",
                "material_contract": "One Mesh0/Primitive0; linear COLOR_0 RGBA; white opaque StandardMaterial",
                "palette_linear_rgba": COLORS, "assets": records,
                "total_glb_bytes": sum(r["bytes"] for r in records)}
    (args.output/"manifest.json").write_text(json.dumps(manifest, indent=2, sort_keys=True)+"\n")
    # Keep six separate, editable source meshes, each at identity. Collections let
    # artists isolate coincident assets without invalidating their export origins.
    for name, obj in objects.items():
        collection = bpy.data.collections.new(name)
        bpy.context.scene.collection.children.link(collection)
        for old in list(obj.users_collection):
            old.objects.unlink(obj)
        collection.objects.link(obj)
        obj.hide_set(name != "jiangting_roof")
    bpy.context.scene.unit_settings.system = 'METRIC'
    bpy.ops.wm.save_as_mainfile(filepath=str((args.output/"riverside.blend").resolve()), compress=True)
    if args.preview:
        preview(objects, args.preview)
    print(f"VERIFIED {len(records)} assets; {manifest['total_glb_bytes']} total GLB bytes")


if __name__ == "__main__":
    main()
