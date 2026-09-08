"""Read-only section sensitivity, never source registration or clearance acceptance."""
import hashlib
import json
import math
from pathlib import Path
from l3_section_study import section_segments
from corner_clearance_study import scene_geometry_digest


def validate_scope(scope, plan):
    source = scope['source_section']
    if (source.get('registration_verified') is not False
            or source.get('resolved_path_xy_m', 'missing') is not None
            or any(scope.get(k) is not False for k in ('export_to_game','transfer_to_exterior'))):
        raise ValueError('Unresolved source path must not be promoted')
    axes = [plan['axes_y_m'][a] for a in source['horizontal_axes_left_to_right']]
    if (axes != [15,11,7,3,-3,-7,-11,-15]
            or source['model_horizontal_direction'] != '-Y'
            or source['right_eave_axis'] != 'B'):
        raise ValueError('Reviewed source axis order must remain M to A')
    bracket = source['ground_plan_markers']['observed_x_bracket_m']
    if bracket != [plan['axes_x_m']['6'],0.]:
        raise ValueError('Marker bracket is axis 6 to centreline, not a cut path')
    diagnostic = scope['diagnostic_cut']
    if (diagnostic.get('kind') != 'fixed_x_mesh_diagnostic'
            or diagnostic['study_version'] != '01' or diagnostic['x_m'] != 3.
            or diagnostic['y_interval_m'] != [8.,15.]
            or diagnostic.get('registered_to_source') is not False):
        raise ValueError('Preserve the historical independent diagnostic')
    probes = scope['sensitivity_probes']
    if (not probes or len({p['id'] for p in probes}) != len(probes)
            or any(p['id'] == 'study_01_control' for p in probes)):
        raise ValueError('Distinct sensitivity probes required')
    for probe in probes:
        x = probe['x_m']
        if (not math.isfinite(x) or not bracket[0] < x < bracket[1]
                or probe['y_interval_m'] != [-15.,-8.]
                or probe.get('registered_to_source') is not False):
            raise ValueError('Hypothetical negative-Y probes only; no source registration')
    return axes


def capture(root):
    """Return actual loop-triangle cuts; caller chooses where to publish evidence."""
    import bpy
    root = Path(root)
    private = root/'.omx/references/yellow-crane'
    scope_path = root/'docs/refactoring/yellow-crane-l3-section-registration.json'
    plan_path = root/'docs/refactoring/yellow-crane-plan-traces.json'
    scope = json.loads(scope_path.read_text())
    axes = validate_scope(scope,json.loads(plan_path.read_text()))
    sha = lambda path: hashlib.sha256(path.read_bytes()).hexdigest()
    for ref in scope['sources']:
        if sha(private/ref['file']) != ref['sha256']:
            raise ValueError('Source plate hash changed')
    source_file = private/'exterior-whitebox-14.blend'
    baseline = json.loads((root/'docs/handoff/yellow-crane/evidence/l3-section-constraint-study-01.json').read_text())
    file_hash = sha(source_file)
    if file_hash != baseline['source_file_sha256']:
        raise ValueError('Expected unchanged v14 checkpoint')
    for scene in bpy.data.scenes:
        for layer in scene.view_layers:
            layer.update()
    active = bpy.context.scene.name
    before = {s.name:scene_geometry_digest(s) for s in bpy.data.scenes}
    scene = bpy.data.scenes['HHL_Exterior_Whitebox_14']
    if before[scene.name] != baseline['before'][scene.name]:
        raise ValueError('Loaded v14 geometry differs from reviewed baseline')
    probes = [{**scope['diagnostic_cut'],'id':'study_01_control'},*scope['sensitivity_probes']]
    results = [{**p,'meshes':{}} for p in probes]
    for label in ('Third_canopy','L3_posts'):
        obj = scene.objects[f'WB14_{label}']
        if obj.modifiers or obj.constraints or obj.parent or obj.animation_data or obj.data.shape_keys:
            raise ValueError('Static unmodified meshes required')
        obj.data.calc_loop_triangles()
        vertices = [tuple(obj.matrix_world@v.co) for v in obj.data.vertices]
        triangles = [[vertices[i] for i in tri.vertices] for tri in obj.data.loop_triangles]
        for result in results:
            segments = section_segments(triangles,result['x_m'],*result['y_interval_m'])
            result['meshes'][label] = {'triangle_count':len(triangles),'segment_count':len(segments),
                                      'segments_yz_m':segments}
    after = {s.name:scene_geometry_digest(s) for s in bpy.data.scenes}
    if before != after or bpy.context.scene.name != active or sha(source_file) != file_hash:
        raise RuntimeError('Read-only preservation failed')
    for label,mesh in results[0]['meshes'].items():
        if mesh['segment_count'] != baseline['mesh_metadata'][label]['section_segments']:
            raise RuntimeError('Historical diagnostic control did not reproduce')
    return {'schema_version':1,'recorded_on':'2026-09-08','actual_blender_readback':True,
            'blender_version':bpy.app.version_string,'source_file_sha256':file_hash,
            'scope_sha256':sha(scope_path),'plan_sha256':sha(plan_path),
            'source_axis_y_m':axes,'probes':results,'before':before,'after':after,
            'active_scene_unchanged':True,'source_file_unchanged':True,
            'registration_verified':False,'clearance_reclassified':False,
            'exterior_geometry_modified':False,'export_to_game':False,
            'limits':scope['limits']}
