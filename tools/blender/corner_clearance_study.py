"""Non-destructive section diagnostics of existing v10 corner conflicts.

Run build(root) via Blender MCP. This creates an annotated analysis scene, NOT an
exterior revision or a roof/post correction. No source images are loaded.
"""
import hashlib
import json
import math
from pathlib import Path
from top_support_whitebox import (
    underside_probe, underside_cap, SUPPORT_WIDTH, SUPPORT_CLEARANCE,
    MIN_SUPPORT_HEIGHT,
)


def scene_geometry_digest(scene):
    """Object transforms, mesh vertices/faces and crown IDs; not all data fields."""
    digest = hashlib.sha256()
    for obj in sorted(scene.objects, key=lambda o: o.name):
        record = [obj.name, obj.type, [list(row) for row in obj.matrix_world]]
        if obj.type == 'MESH':
            record.extend(([list(v.co) for v in obj.data.vertices],
                           [list(p.vertices) for p in obj.data.polygons]))
            region = obj.data.attributes.get('crown_region')
            if region:
                record.append([item.value for item in region.data])
        digest.update(json.dumps(record, separators=(',', ':')).encode())
    return digest.hexdigest()


def build(root, version='11'):
    import bpy
    from exterior_whitebox import save_checkpoint
    root = Path(root)
    directory = root / '.omx/references/yellow-crane'
    stem = f'corner-clearance-study-{version}'
    blend, report = directory / f'{stem}.blend', directory / f'{stem}.json'
    image = directory / f'{stem}.png'
    name = f'HHL_Corner_Clearance_Study_{version}'
    if name in bpy.data.scenes or any(p.exists() for p in (blend, report, image)):
        raise FileExistsError('Preserve the existing study; use a fresh version')
    source = bpy.data.scenes['HHL_Exterior_Whitebox_10']
    before = {s.name: scene_geometry_digest(s) for s in bpy.data.scenes}
    source_file = directory / 'exterior-whitebox-10.blend'
    file_digest = hashlib.sha256(source_file.read_bytes()).hexdigest()
    roof = source.objects['WB10_Fifth_lower_canopy']
    roof.data.calc_loop_triangles()
    triangles = [tuple(tuple(roof.matrix_world @ roof.data.vertices[i].co)
                       for i in t.vertices) for t in roof.data.loop_triangles]
    posts = source.objects['WB10_L5_posts']
    vertices = [posts.matrix_world @ v.co for v in posts.data.vertices]
    blocked = json.loads(source['support_proxy_review'])['blocked']
    if len(blocked) != 4:
        raise ValueError('Expected v10 four-corner baseline; recheck source before drawing')
    rows = []
    for item in blocked:
        x, y = item['center']
        post_head = max(v.z for v in vertices
                        if abs(v.x-x) < SUPPORT_WIDTH/2 and abs(v.y-y) < SUPPORT_WIDTH/2)
        probe = underside_probe(triangles, (x, y), SUPPORT_WIDTH)
        wx, wy, wz = probe['minimum_point']
        distance = math.hypot(wx-x, wy-y)
        if distance < 1e-8:
            raise ValueError('Minimum at center; this section direction is undefined')
        required = post_head + SUPPORT_CLEARANCE + MIN_SUPPORT_HEIGHT
        # 1mm square probes approximate this radial section, not an exact ray.
        profile = []
        for i in range(41):
            d = distance*i/40
            z = underside_cap(triangles, (x+(wx-x)*i/40, y+(wy-y)*i/40), .001)
            profile.append([d, z])
        rows.append({'center': [x, y], 'post_head_m': post_head,
                     'required_cap_strictly_above_m': required,
                     'minimum': probe, 'fit_margin_m': wz-required,
                     'section_length_m': distance, 'section_probe_width_m': .001,
                     'sampled_section': profile})

    scene = bpy.data.scenes.new(name)
    scene['status'] = 'DIAGNOSTIC_ONLY_NOT_EXTERIOR_REVISION'
    scene['export_to_game'] = False
    scene.world = bpy.data.worlds.new(f'{stem}_world')
    scene.world.color = (.018, .024, .038)
    scene.render.engine = 'BLENDER_WORKBENCH'
    scene.display.shading.light = 'FLAT'
    scene.display.shading.color_type = 'OBJECT'
    scene.display.shading.show_shadows = False
    scene.display.shading.show_cavity = False
    scene.display.shading.background_type = 'WORLD'
    scene.render.resolution_x = 1600
    scene.render.resolution_y = 1200
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = 'PNG'
    scene.render.filepath = str(image)
    white, muted = (.85,.89,.95,1), (.43,.51,.64,1)
    amber, red, blue = (1,.66,.12,1), (1,.17,.22,1), (.16,.64,1,1)

    def line(label, points, color, width=.012):
        curve = bpy.data.curves.new(label, 'CURVE')
        curve.dimensions = '3D'
        curve.bevel_depth = width
        curve.bevel_resolution = 0
        spline = curve.splines.new('POLY')
        spline.points.add(len(points)-1)
        for point, (x,y) in zip(spline.points, points):
            point.co = (x,y,0,1)
        obj = bpy.data.objects.new(label, curve)
        obj.color = color
        scene.collection.objects.link(obj)

    def text(label, body, x, y, size=.14, color=white):
        font = bpy.data.curves.new(label, 'FONT')
        font.body, font.size = body, size
        obj = bpy.data.objects.new(label, font)
        obj.location = (x,y,.04)
        obj.color = color
        scene.collection.objects.link(obj)

    text('Title', 'HHL / CORNER CLEARANCE', -4.3, 3.15, .29)
    text('Subtitle', 'V10 geometry unchanged  |  Diagnostic study 11  |  NOT a construction drawing', -4.3, 2.82, .13, muted)
    text('Legend', 'AMBER: sampled roof underside     RED: minimum fit threshold     BLUE: existing post head', -4.3, 2.54, .12)
    for n, row in enumerate(rows):
        ox, oy = (-4.2 + (n%2)*4.5, .24 - (n//2)*2.45)
        head, required = row['post_head_m'], row['required_cap_strictly_above_m']
        # Deliberately non-uniform plot axes, labeled below; not a 1:1 section.
        project = lambda d,z: (ox+3.6*d/row['section_length_m'], oy+.24+(z-head)*2.4)
        for label, z, color in [('Post',head,blue), ('Threshold',required,red)]:
            line(f'{n}_{label}', [project(0,z),project(row['section_length_m'],z)], color, .008)
        line(f'{n}_Roof', [project(d,z) for d,z in row['sampled_section']], amber)
        px, py = project(row['section_length_m'], row['minimum']['cap_m'])
        line(f'{n}_Witness', [(px-.045,py-.045),(px+.045,py+.045)], red, .014)
        line(f'{n}_Witness2', [(px-.045,py+.045),(px+.045,py-.045)], red, .014)
        x,y = row['center']
        text(f'{n}_Heading', f'CORNER ({x:+g}, {y:+g})', ox, oy+2.00, .17)
        text(f'{n}_Margin', f"Fit margin: {row['fit_margin_m']*1000:.1f} mm  /  BLOCKED", ox, oy+1.76, .145, red)
        text(f'{n}_Readout', f"Min cap {row['minimum']['cap_m']:.4f} m  /  required > {required:.3f} m", ox, oy-.02, .125)
        text(f'{n}_Axis', f"Center to witness: {row['section_length_m']:.3f} m  |  axes not 1:1", ox, oy-.24, .12, muted)
    text('Warning', '37.300 m is a dark/mezzanine level, NOT a verified corner post head. Do not lower posts to pass.', -4.3, -3.0, .125, white)
    text('Limits', '1.04 m square proxy  |  20 mm clearance + >40 mm height  |  Single-layer roof assumption  |  No geometry corrected', -4.3, -3.26, .115, muted)
    camera = bpy.data.cameras.new(f'{stem}_camera')
    obj = bpy.data.objects.new(camera.name, camera)
    scene.collection.objects.link(obj)
    obj.location = (0,0,20)
    camera.type = 'ORTHO'
    camera.ortho_scale = 9.5
    scene.camera = obj
    after = {s: scene_geometry_digest(bpy.data.scenes[s]) for s in before}
    if before != after:
        raise AssertionError('Original scene geometry changed')
    payload = {'status': 'diagnostic_complete_corners_unresolved', 'source_scene': source.name,
               'scene': name, 'rows': rows, 'prior_scene_count': len(before),
               'before': before, 'after': after, 'prior_scene_geometry_unchanged': before == after,
               'source_file_sha256': file_digest,
               'source_file_unchanged': hashlib.sha256(source_file.read_bytes()).hexdigest() == file_digest,
               'limits': ['Existing v10 fit policy, not structural criteria',
                          'Whole-square minimum exact for actual piecewise-planar triangles under single-layer assumption',
                          'Plotted profile uses 41 one-millimeter square probes; non-uniform display axes',
                          '37.300 m section level is not evidence for a replacement post head',
                          'Preservation covers transforms, mesh vertices/faces and crown IDs, not every data field']}
    if not payload['source_file_unchanged']:
        raise AssertionError('Original saved v10 file changed')
    bpy.context.window.scene = scene
    save_checkpoint(blend, report, payload,
                    lambda: bpy.ops.wm.save_as_mainfile(filepath=str(blend), compress=True))
    bpy.ops.render.render(write_still=True, scene=scene.name)
    return {'scene': name, 'blend': str(blend), 'report': str(report), 'image': str(image),
            'blocked_corners': len(rows), 'old_scenes_preserved': len(before)}
