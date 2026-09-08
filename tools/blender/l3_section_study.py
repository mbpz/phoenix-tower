"""Separate source-datum / actual mesh section; never changes exterior geometry.

Run build(root) in Blender with v14 loaded. The two panels share a Z scale,
NOT a source-plan correspondence. No reference images are imported; the saved
private checkpoint retains the original scenes and is not a portable asset.
"""
import hashlib
import json
import math
from pathlib import Path
from corner_clearance_study import scene_geometry_digest
from exterior_whitebox import save_checkpoint


def source_datums(data, eave_detail):
    """Validate integer-mm closure and independent transcription, not semantics."""
    flags = ('column_top_verified', 'transfer_to_exterior',
             'inner_ring_surface_verified', 'plan_correspondence_verified')
    if any(data.get(key) is not False for key in flags):
        raise ValueError('Unresolved source meanings cannot be promoted by this study')
    section, detail = data['section'], data['detail']
    chain = section['descending_chain_mm']
    if not chain or any(type(v) is not int or v <= 0 for v in chain):
        raise ValueError('Positive integer millimetres required')
    top, bottom = section['top_m'], section['bottom_m']
    if not all(math.isfinite(v) for v in (top,bottom)) or top <= bottom:
        raise ValueError('Finite descending station elevations required')
    stations = [round(top*1000)]
    for step in chain:
        stations.append(stations[-1]-step)
    if abs(stations[0]/1000-top)>1e-9 or abs(stations[-1]/1000-bottom)>1e-9:
        raise ValueError('Section dimension chain does not close')
    if len(section['station_meanings']) != len(stations):
        raise ValueError('Each station needs an explicit meaning/uncertainty')
    independent = next(r for r in eave_detail['symmetric_elevations_m'] if r['floor']=='L3')
    for key, other in [('tip_m','upper_left'),('upper_right_m','upper_right'),('low_eave_m','lower_right')]:
        if detail[key] != independent[other]:
            raise ValueError('Independent eave transcription mismatch')
    drop = sum(detail['descending_chain_mm'])/1000
    if (detail['descending_chain_mm'] != list(reversed(eave_detail['section_vertical_chain_mm']))
            or abs(detail['upper_right_m']-detail['low_eave_m']-drop)>1e-9
            or abs(stations[1]/1000-detail['upper_right_m'])>1e-9):
        raise ValueError('Independent detail chain mismatch')
    return {'stations_m':[v/1000 for v in stations], 'detail_drop_m':drop,
            **{key:False for key in flags}}


def post_readout(datums, post_top):
    """Keep numeric evidence and its annotation tied to the same source station."""
    station = datums['stations_m'][2]
    if not all(math.isfinite(v) for v in (station,post_top)):
        raise ValueError('Finite source station and actual post top required')
    delta = station-post_top
    return {'support_station_m':station, 'estimated_post_top_m':post_top,
            'support_station_minus_estimated_post_top_m':delta,
            'label':f'Post top {post_top:.3f} m vs datum {station:.3f} m: {delta*1000:+.1f} mm'}


def section_segments(triangles, x, lo, hi):
    """Intersect triangles with X=x, clip in world Y; return (Y,Z) segments.

    Coplanar triangles are ambiguous area sections and rejected. Tangencies have
    no length. Duplicated shared-edge segments are merged at nanometre rounding.
    This is wire geometry, not overlap classification or a fitted roof curve.
    """
    if not all(math.isfinite(v) for v in (x,lo,hi)) or lo >= hi:
        raise ValueError('Finite plane and increasing Y interval required')
    segments = set()
    for tri in triangles:
        if len(tri)!=3 or any(len(p)!=3 or not all(math.isfinite(v) for v in p) for p in tri):
            raise ValueError('Finite XYZ triangles required')
        distances = [p[0]-x for p in tri]
        if all(abs(d)<1e-9 for d in distances):
            raise ValueError('Coplanar triangle: choose a different section plane')
        points = []
        for i, a in enumerate(tri):
            j = (i+1)%3
            b, da, db = tri[j], distances[i], distances[j]
            if abs(da)<1e-9:
                points.append((a[1],a[2]))
            if da*db<0 and abs(da)>=1e-9 and abs(db)>=1e-9:
                t = da/(da-db)
                points.append((a[1]+t*(b[1]-a[1]),a[2]+t*(b[2]-a[2])))
        points = sorted(set((round(y,9),round(z,9)) for y,z in points))
        if len(points)<2:
            continue
        if len(points)!=2:
            raise ValueError('Ambiguous triangle section')
        a,b = points
        if b[0]<lo or a[0]>hi:
            continue
        if b[0]-a[0]>1e-9:
            start,stop = max(a[0],lo),min(b[0],hi)
            interpolate = lambda y: (y,a[1]+(b[1]-a[1])*(y-a[0])/(b[0]-a[0]))
            a,b = interpolate(start),interpolate(stop)
        if math.dist(a,b)>1e-9:
            segments.add(tuple((round(y,9),round(z,9)) for y,z in (a,b)))
    return sorted(segments)


def build(root, version='01'):
    import bpy
    root = Path(root)
    directory = root / '.omx/references/yellow-crane'
    stem = f'l3-section-constraint-study-{version}'
    blend, report, image = [directory/f'{stem}.{ext}' for ext in ('blend','json','png')]
    name = f'HHL_L3_Section_Constraint_Study_{version}'
    if name in bpy.data.scenes or any(p.exists() for p in (blend,report,image)):
        raise FileExistsError('Preserve existing studies; use a fresh version')
    constraints_path = root/'docs/refactoring/yellow-crane-l3-section-constraints.json'
    data = json.loads(constraints_path.read_text())
    datums = source_datums(data,json.loads((root/'docs/refactoring/yellow-crane-eave-traces.json').read_text()))
    for ref in data['sources']:
        if hashlib.sha256((directory/ref['file']).read_bytes()).hexdigest()!=ref['sha256']:
            raise ValueError('Local reference hash differs from reviewed transcription')
    source = bpy.data.scenes['HHL_Exterior_Whitebox_14']
    source_file = directory/'exterior-whitebox-14.blend'
    source_hash = hashlib.sha256(source_file.read_bytes()).hexdigest()
    for scene in bpy.data.scenes:
        for layer in scene.view_layers:
            layer.update()
    before = {s.name:scene_geometry_digest(s) for s in bpy.data.scenes}
    sections, metadata = {}, {}
    plane_x, lo, hi = 3.,8.,15.
    for label in ('Third_canopy','L3_posts'):
        obj = source.objects[f'WB14_{label}']
        if obj.modifiers or obj.constraints or obj.parent or obj.animation_data or obj.data.shape_keys:
            raise ValueError('Static, unmodified mesh required for section readback')
        obj.data.calc_loop_triangles()
        vertices = [tuple(obj.matrix_world@v.co) for v in obj.data.vertices]
        triangles = [[vertices[i] for i in t.vertices] for t in obj.data.loop_triangles]
        sections[label] = section_segments(triangles,plane_x,lo,hi)
        if not sections[label]:
            raise ValueError('Expected actual roof and post sections')
        metadata[label] = {'triangles':len(triangles),'section_segments':len(sections[label]),
                           'maximum_z_m':max(p[2] for p in vertices)}
    readout = post_readout(datums,metadata['L3_posts']['maximum_z_m'])
    scene = bpy.data.scenes.new(name)
    scene['status'] = 'SOURCE_DATUM_AND_ACTUAL_V14_SECTION_ONLY'
    scene['export_to_game'] = False
    scene['source_plan_correspondence_verified'] = False
    scene.world = bpy.data.worlds.new(stem+'_world')
    scene.world.color = (.018,.024,.038)
    scene.render.engine = 'BLENDER_WORKBENCH'
    shading = scene.display.shading
    shading.light, shading.color_type, shading.background_type = 'FLAT','OBJECT','WORLD'
    shading.show_shadows = shading.show_cavity = False
    scene.render.resolution_x, scene.render.resolution_y = 1800,1100
    scene.render.resolution_percentage = 100
    scene.render.image_settings.file_format = 'PNG'
    scene.render.filepath = str(image)
    white, muted, amber, blue = (.9,.94,1,1),(.5,.61,.75,1),(1,.68,.2,1),(.2,.68,1,1)
    def line(label, points, color, width=.008):
        curve = bpy.data.curves.new(label,'CURVE')
        curve.dimensions,curve.bevel_depth = '3D',width
        curve.bevel_resolution = 0
        spline = curve.splines.new('POLY'); spline.points.add(len(points)-1)
        for point,(x,y) in zip(spline.points,points): point.co=(x,y,0,1)
        obj=bpy.data.objects.new(label,curve); obj.color=color; scene.collection.objects.link(obj)
    def text(label, body, x, y, size=.13, color=white):
        font=bpy.data.curves.new(label,'FONT'); font.body,font.size=body,size
        obj=bpy.data.objects.new(label,font); obj.location=(x,y,.04); obj.color=color
        scene.collection.objects.link(obj)
    text('Title','L3 / SOURCE DATUM + ACTUAL MESH SECTION',-6.3,3.25,.28)
    text('Scope','Independent study  |  V14 unchanged  |  NO construction or game approval',-6.3,2.91,.15,muted)
    text('Source','A / 2-2-5 printed vertical chain',-6.3,2.48,.18)
    text('Actual','B / V14 actual loop triangles: X = +3 m',-.75,2.48,.18)
    text('SectionScope','World Y = 8 to 15 m; NOT registered to source section XY',-.75,2.19,.13,muted)
    # Both panels use world metres vertically. Only panel B has a plan X axis.
    project_z=lambda z: z-24.3
    for i,z in enumerate(datums['stations_m']):
        y=project_z(z)
        line(f'Datum_{i}',[(-6.25,y),(-1.15,y)],muted,.005)
        text(f'Station_{i}',f'{z:.3f} m',-6.2,y+.07,.15)
        if i<len(data['section']['descending_chain_mm']):
            step=data['section']['descending_chain_mm'][i]
            text(f'Dimension_{i}',f'{step} mm',-2.0,y-step/2000-.045,.12,amber)
    text('Meaning0','L4 floor datum',-4.75,project_z(datums['stations_m'][0])+.07,.13)
    text('Meaning1','Local connection candidate',-4.75,project_z(datums['stations_m'][1])+.07,.13,amber)
    text('Meaning2','Support-region datum / NOT post top',-4.75,project_z(readout['support_station_m'])+.07,.12,amber)
    for label,color in [('Third_canopy',amber),('L3_posts',blue)]:
        for i,segment in enumerate(sections[label]):
            # Clip the visible window vertically; full segments remain in source mesh.
            (a,z1),(b,z2)=segment
            if max(z1,z2)<22.2 or min(z1,z2)>26.1: continue
            if abs(z2-z1)>1e-9:
                t0,t1=sorted(((22.2-z1)/(z2-z1),(26.1-z1)/(z2-z1)))
                lower,upper=max(0.,t0),min(1.,t1)
                if lower>upper: continue
                ends=[(a+(b-a)*t,z1+(z2-z1)*t) for t in (lower,upper)]
            else: ends=segment
            line(f'{label}_section_{i}',[(y-lo-.75,project_z(z)) for y,z in ends],color)
    for y in range(8,16):
        x=y-lo-.75
        text(f'Axis_{y}',str(y),x-.05,-2.36,.12,muted)
    text('Legend','AMBER roof shell   /   BLUE estimated post   /   Y metres',-.75,-2.67,.135)
    text('Readout',readout['label'],-.75,-2.96,.14,blue)
    text('NoGo','A datum is not a replacement height. No roof/post dimensions changed.',-6.3,-3.35,.17,white)
    text('Detail','2-2-14: tip 25.600 / upper-right 25.400 / low-eave 23.820 m; XY + surface mapping unresolved.',-6.3,-3.63,.135,muted)
    camera=bpy.data.cameras.new(stem+'_camera'); obj=bpy.data.objects.new(camera.name,camera)
    scene.collection.objects.link(obj); obj.location=(0,-.12,20)
    camera.type,camera.ortho_scale='ORTHO',13.8; scene.camera=obj
    after={s:scene_geometry_digest(bpy.data.scenes[s]) for s in before}
    if before!=after or hashlib.sha256(source_file.read_bytes()).hexdigest()!=source_hash:
        raise AssertionError('Source geometry or source file changed')
    payload={'schema_version':1,'recorded_on':'2026-09-08','status':'study_complete_mapping_unresolved',
             'scene':name,'source_scene':source.name,'source_file_sha256':source_hash,
             'constraints_sha256':hashlib.sha256(constraints_path.read_bytes()).hexdigest(),
             'source_datums':datums,'actual_blender_readback':True,'blender_version':bpy.app.version_string,
             'section_plane_x_m':plane_x,'section_y_interval_m':[lo,hi],'mesh_metadata':metadata,
             **{k:v for k,v in readout.items() if k!='label'},
             'prior_scene_count':len(before),'before':before,'after':after,
             'prior_scene_geometry_unchanged':True,'source_file_unchanged':True,
             'exterior_geometry_modified':False,'export_to_game':False,
             'limits':data['limits']+['Panel B is an actual triangle section, NOT registered to the source section XY.',
                                    'No new contact pass/fail or global solid-intersection validation.',
                                    'Preservation digests cover transforms and mesh geometry, not every datablock property.']}
    # Reuse the existing private checkpoint path; never overwrite the source.
    # libraries.write(scene) crashed in the local Blender build during a prior
    # attempt. Keep the full private file rather than claiming a portable export.
    original_scene = bpy.context.window.scene
    try:
        bpy.context.window.scene = scene
        scene.view_layers[0].update()
        save_checkpoint(blend,report,payload,
                        lambda: bpy.ops.wm.save_as_mainfile(filepath=str(blend),compress=True))
        bpy.ops.render.render(write_still=True,scene=scene.name)
    finally:
        bpy.context.window.scene = original_scene
    return {'scene':name,'blend':str(blend),'report':str(report),'image':str(image)}
