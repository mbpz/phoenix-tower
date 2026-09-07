"""Independent pixel-trace inspection scene; run through Blender MCP.

No conversion to roof metric geometry is permitted until plan-developed
lengths and the conflicting printed elevations have been reconciled.
"""
import json
from pathlib import Path


def display_points(data):
    x0, y0, x1, y1 = data['display']['crop_bounds_px']
    scale = data['display']['pixels_per_display_unit']
    if scale <= 0 or x1 <= x0 or y1 <= y0:
        raise ValueError('Invalid reference display frame')
    return [((x-(x0+x1)/2)/scale, ((y0+y1)/2-y)/scale, .02)
            for x,y in data['asymmetric_lower_curve']['points_px']]


def build_eave_study(root):
    import bpy
    from mathutils import Quaternion, Vector

    root = Path(root)
    data = json.loads((root/'docs/refactoring/yellow-crane-eave-traces.json').read_text())
    if data['export_to_game'] or data['asymmetric_lower_curve']['metric_points'] is not None:
        raise ValueError('Pixel study must remain independent from metric roof geometry')
    name = 'HHL_Eave_Pixel_Study'
    if name in bpy.data.scenes:
        raise RuntimeError('Preserve existing eave study; do not overwrite it')
    image_path = root/'.omx/references/yellow-crane'/data['display']['preview_file']
    if not image_path.is_file():
        raise FileNotFoundError(image_path)
    points = display_points(data)
    scene = bpy.data.scenes.new(name)
    scene.unit_settings.system = 'NONE'
    scene['export_to_game'] = False
    scene['status'] = data['status']
    scene['unit_warning'] = data['display']['units']
    coll = bpy.data.collections.new('EAVE_PIXEL_REFERENCE_ONLY')
    scene.collection.children.link(coll)
    coll['export_to_game'] = False
    ref = bpy.data.objects.new('REF_2_2_14_Asymmetric', None)
    ref.empty_display_type = 'IMAGE'
    ref.data = bpy.data.images.load(str(image_path), check_existing=True)
    bounds = data['display']['crop_bounds_px']
    ref.empty_display_size = (bounds[2]-bounds[0])/data['display']['pixels_per_display_unit']
    ref.empty_image_offset = (-.5, -.5)
    ref.empty_image_depth = 'BACK'
    ref.empty_image_side = 'DOUBLE_SIDED'
    ref.location.z = -.01
    coll.objects.link(ref)

    curve = bpy.data.curves.new('Asymmetric_14_Manual_Samples', 'CURVE')
    curve.dimensions = '3D'
    curve.bevel_depth = .014
    curve.bevel_resolution = 0
    poly = curve.splines.new('POLY')
    poly.points.add(len(points)-1)
    for point, co in zip(poly.points, points):
        point.co = (*co, 1)
    obj = bpy.data.objects.new(curve.name, curve)
    coll.objects.link(obj)
    obj.color = (.015, .20, .90, 1)
    obj['source_figure'] = data['source']['figure']
    obj['metric_roof_mapping'] = 'UNRESOLVED; not a radial sweep profile'
    obj['source_points_px'] = json.dumps(data['asymmetric_lower_curve']['points_px'])
    obj['conflicts'] = json.dumps(data['unresolved_conflicts'])

    label = bpy.data.curves.new('Eave_Study_Notice', 'FONT')
    label.body = 'ASYMMETRIC EAVE / PIXEL TRACE ONLY\nNOT METRIC - NOT FOR ROOF SWEEP\nL2 lower: 17.200 vs 17.220 m; L4 symmetric lower: unresolved'
    label.size = .18
    note = bpy.data.objects.new(label.name, label)
    note.location = (-5, 3.75, 0)
    note.color = (.82, .9, 1, 1)
    coll.objects.link(note)
    for item in coll.objects:
        item['export_to_game'] = False
    camera_data = bpy.data.cameras.new(name+'_Camera')
    camera_data.type = 'ORTHO'
    camera_data.ortho_scale = 13
    camera_obj = bpy.data.objects.new(camera_data.name, camera_data)
    scene.collection.objects.link(camera_obj)
    camera_obj.location = (0, .4, 20)
    scene.camera = camera_obj  # Default -Z direction looks at XY reference.
    scene.render.resolution_x = 1600
    scene.render.resolution_y = 1000
    bpy.context.window.scene = scene
    for area in bpy.context.screen.areas:
        if area.type == 'VIEW_3D':
            space = area.spaces.active
            space.shading.type = 'SOLID'
            space.shading.color_type = 'OBJECT'
            space.overlay.show_overlays = True
            space.overlay.show_floor = False
            space.overlay.show_axis_x = False
            space.overlay.show_axis_y = False
            space.overlay.show_extras = True  # image empties must remain visible
            space.region_3d.view_perspective = 'ORTHO'
            space.region_3d.view_rotation = Quaternion((1, 0, 0, 0))
            space.region_3d.view_location = Vector((0, .4, 0))
            space.region_3d.view_distance = 13
    return {'scene': name, 'sample_count': len(points), 'metric': False,
            'unresolved_conflicts': len(data['unresolved_conflicts'])}
