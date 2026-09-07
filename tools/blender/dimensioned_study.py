"""Build the local, non-exportable HHL design study through Blender MCP.

Geometry helpers are pure Python. bpy is imported only by build_study().
All scenes are new; a repeated invocation refuses to overwrite user edits.
Column XY/elevations derive from design plates. Member lengths, diameters,
slab thickness and finished edges are explicitly schematic, not as-built.
"""
import json
import math
from pathlib import Path


def inside(x, y, polygon):
    result = False
    for (ax, ay), (bx, by) in zip(polygon, polygon[1:] + polygon[:1]):
        if (ay > y) != (by > y) and x < (bx-ax)*(y-ay)/(by-ay)+ax:
            result = not result
    return result


def slab_mesh(polygon, holes, z, thickness):
    """Orthogonal grid tessellation with shared vertices and real through-holes."""
    if thickness <= 0 or len(polygon) < 4:
        raise ValueError('A positive thickness and orthogonal outline are required')
    for a, b in zip(polygon, polygon[1:] + polygon[:1]):
        if a[0] != b[0] and a[1] != b[1]:
            raise ValueError('Slab outlines must be orthogonal')
    xs = sorted({p[0] for p in polygon} | {h['rect_m'][i] for h in holes for i in (0, 2)})
    ys = sorted({p[1] for p in polygon} | {h['rect_m'][i] for h in holes for i in (1, 3)})
    verts, faces, cache, boundary = [], [], {}, {}

    def vertex(x, y, height):
        key = (round(x, 9), round(y, 9), round(height, 9))
        if key not in cache:
            cache[key] = len(verts)
            verts.append(key)
        return cache[key]

    for x0, x1 in zip(xs, xs[1:]):
        for y0, y1 in zip(ys, ys[1:]):
            x, y = (x0+x1)/2, (y0+y1)/2
            if not inside(x, y, polygon):
                continue
            if any(a < x < c and b < y < d for a,b,c,d in (h['rect_m'] for h in holes)):
                continue
            corners = [(x0,y0),(x1,y0),(x1,y1),(x0,y1)]
            top = [vertex(a,b,z) for a,b in corners]
            bottom = [vertex(a,b,z-thickness) for a,b in corners]
            faces.extend([top, list(reversed(bottom))])
            for i in range(4):
                j = (i+1) % 4
                key = tuple(sorted((top[i], top[j])))
                if key in boundary:
                    del boundary[key]
                else:
                    boundary[key] = [bottom[i], bottom[j], top[j], top[i]]
    faces.extend(boundary.values())
    return verts, faces


def column_mesh(centres, bottom, top, radius, sides=16):
    if top <= bottom or radius <= 0 or sides < 3:
        raise ValueError('Invalid column display dimensions')
    verts, faces = [], []
    for x, y in centres:
        n = len(verts)
        for z in (bottom, top):
            verts.extend((x+radius*math.cos(2*math.pi*i/sides),
                          y+radius*math.sin(2*math.pi*i/sides), z) for i in range(sides))
        faces.extend([[n+i,n+(i+1)%sides,n+sides+(i+1)%sides,n+sides+i] for i in range(sides)])
        faces.extend([list(reversed(range(n,n+sides))),list(range(n+sides,n+2*sides))])
    return verts, faces


def validate_trace(data):
    if data['export_to_game'] is not False:
        raise ValueError('Study must remain non-exportable')
    ids = set()
    for f in data['floors']:
        if f['id'] in ids:
            raise ValueError('Duplicate floor identifier')
        ids.add(f['id'])
        points=[]
        for row, columns in f['column_axes_by_row'].items():
            for col in columns:
                points.append((data['axes_x_m'][str(col)], data['axes_y_m'][row]))
        if len(points) != len(set(points)) or len(points) != f['column_count']:
            raise ValueError('Column occupancy mismatch')
        if f['schematic_post_top_m'] <= f['elevation_m']:
            raise ValueError('Invalid schematic height')
        c=data['calibrations'][f['figure']]
        if any(b <= a for a,b in (c['x_endpoints_px'], c['y_endpoints_px'])):
            raise ValueError('Invalid calibration endpoints')


def reference_display_size(calibration):
    """Blender image empties size their longest side, including portrait plates."""
    c = calibration
    return max(c['image_size_px']) * c['span_m'] / (c['x_endpoints_px'][1]-c['x_endpoints_px'][0])


def study_output_paths(root):
    """Refuse collisions across Blender sessions, before touching any scene."""
    refs = Path(root) / '.omx/references/yellow-crane'
    paths = (refs / 'dimensioned-structure-study.blend',
             refs / 'structure-study-validation.json')
    for path in paths:
        if path.exists():
            raise FileExistsError(f'Preserve existing study output: {path}; use a new versioned workspace')
    return paths


def build_study(root):
    import bpy
    from mathutils import Vector

    root = Path(root)
    blend_path, report_path = study_output_paths(root)
    refs = root / '.omx/references/yellow-crane'
    data = json.loads((root / 'docs/refactoring/yellow-crane-plan-traces.json').read_text())
    validate_trace(data)
    names = ['HHL_Structure_Study', 'HHL_Exploded_Study', 'HHL_Plan_Trace_QA']
    if any(name in bpy.data.scenes for name in names):
        raise RuntimeError('Study scene already exists; preserve it and explicitly version the next build')
    for c in data['calibrations'].values():
        if not (refs / c['image']).is_file():
            raise FileNotFoundError(refs / c['image'])

    def scene(name):
        s=bpy.data.scenes.new(name)
        s.unit_settings.system='METRIC'
        s.unit_settings.scale_length=1
        s['status']=data['status']
        s['export_to_game']=False
        s['proxy_warning']='Post heights/radii and slab thickness are schematic. Roofs, stairs, walls and finial incomplete.'
        s.render.engine='BLENDER_WORKBENCH'
        s.render.resolution_x=1600
        s.render.resolution_y=1400
        s.render.resolution_percentage=100
        sh=s.display.shading
        sh.light='STUDIO'
        sh.color_type='OBJECT'
        sh.show_shadows=True
        sh.show_cavity=True
        sh.cavity_type='BOTH'
        sh.curvature_ridge_factor=1.5
        sh.curvature_valley_factor=1.2
        sh.show_specular_highlight=True
        sh.background_type='WORLD'
        s.world=bpy.data.worlds.new(name+'_World')
        s.world.color=(.035,.048,.065)
        return s

    stacked, exploded, qa = [scene(n) for n in names]
    def collection(s,name):
        c=bpy.data.collections.new(name)
        s.collection.children.link(c)
        c['export_to_game']=False
        return c

    def mesh(name, geometry, coll, color, floor, kind):
        verts,faces=geometry
        m=bpy.data.meshes.new(name)
        m.from_pydata(verts,[],faces)
        if m.validate(verbose=False):
            raise ValueError('Generated mesh needed repair: '+name)
        m.update()
        o=bpy.data.objects.new(name,m)
        coll.objects.link(o)
        o.color=color
        o['source_figure']=floor['figure']
        o['source_floor']=floor['id']
        o['elevation_m']=floor['elevation_m']
        o['export_to_game']=False
        o['evidence_status']=data['status']
        o['geometry_kind']=kind
        o['proxy_parameters']=json.dumps(data['display_proxies'],ensure_ascii=False)
        o['outline_meaning']=floor['outline_meaning']
        o['selected_plan_column_count']=floor['column_count']
        return o

    def curve(name, paths, coll, color, radius=.035):
        c=bpy.data.curves.new(name,'CURVE')
        c.dimensions='3D'
        c.bevel_depth=radius
        c.bevel_resolution=0
        for points,closed in paths:
            sp=c.splines.new('POLY')
            sp.points.add(len(points)-1)
            for p,co in zip(sp.points,points):
                p.co=(*co,1)
            sp.use_cyclic_u=closed
        o=bpy.data.objects.new(name,c)
        coll.objects.link(o)
        o.color=color
        o['export_to_game']=False
        return o

    def label(coll,text,location,size=1,rotation=None):
        d=bpy.data.curves.new('Study_Label','FONT')
        d.body=text
        d.size=size
        d.align_x='LEFT'
        o=bpy.data.objects.new('NOTE_'+text.split('\n')[0],d)
        coll.objects.link(o)
        o.location=location
        if rotation is not None:
            o.rotation_euler=rotation
        o.color=(.7,.8,.92,1)
        o['export_to_game']=False
        return o

    def camera(s,position,target,scale):
        d=bpy.data.cameras.new(s.name+'_Camera')
        d.type='ORTHO'
        d.ortho_scale=scale
        o=bpy.data.objects.new(d.name,d)
        s.collection.objects.link(o)
        o.location=position
        o.rotation_euler=(Vector(target)-o.location).to_track_quat('-Z','Y').to_euler()
        s.camera=o
        return o

    cam=camera(stacked,(64,-86,64),(0,0,18), 74)
    ecam=camera(exploded,(70,-95,104),(0,0,34),110)
    camera(qa,(58,-31,150),(58,-31,0),200)
    qa.render.resolution_x=2000
    qa.render.resolution_y=1350
    meta=collection(stacked,'STUDY_NOTES_NOT_FOR_EXPORT')
    emeta=collection(exploded,'EXPLODED_NOTES_NOT_FOR_EXPORT')
    metrics=[]
    for index,f in enumerate(data['floors']):
        z=f['elevation_m']
        coll=collection(stacked,'STUDY_'+f['id'])
        ecoll=collection(exploded,'EXPLODED_'+f['id'])
        centres=[(data['axes_x_m'][str(col)],data['axes_y_m'][row]) for row,cols in f['column_axes_by_row'].items() for col in cols]
        plate=mesh(f['id']+'_Floor_Envelope',slab_mesh(f['outline_m'],f['holes'],z,data['display_proxies']['slab_thickness_m']),coll,(.77,.74,.65,1),f,'floor_envelope_proxy_with_openings')
        posts=mesh(f['id']+'_Schematic_Posts',column_mesh(centres,z,f['schematic_post_top_m'],data['display_proxies']['post_radius_m']),coll,(.53,.31,.22,1),f,'verified_XY_schematic_members')
        for p in posts.data.polygons:
            p.use_smooth=len(p.vertices)==4
        boundary=curve(f['id']+'_Reference_Perimeter',[([(x,y,z+.04) for x,y in f['outline_m']],True)],coll,(.12,.49,.7,1),.045)
        boundary['meaning']=f['outline_meaning']
        for obj in (plate,posts,boundary):
            copy=obj.copy()
            ecoll.objects.link(copy)
            copy.location.z=index*5.5
            copy['display_z_offset_m']=index*5.5
        metrics.append({'floor':f['id'],'selected_columns':len(centres),'floor_vertices':len(plate.data.vertices),'column_vertices':len(posts.data.vertices),'triangles':sum(len(p.vertices)-2 for o in (plate,posts) for p in o.data.polygons)})
        for target_coll,camera_obj,height in [(meta,cam,z),(emeta,ecam,z+index*5.5)]:
            loc=Vector((22,0,height))
            rotation=(camera_obj.location-loc).to_track_quat('Z','Y').to_euler()
            label(target_coll,f"{f['id']}   +{z:06.3f} m",loc,.95,rotation)

        # Plan QA contains reference-only curves, not opaque floor meshes.
        qcoll=collection(qa,'PLAN_QA_'+f['id'])
        c=data['calibrations'][f['figure']]
        w,h=c['image_size_px']
        sx=c['span_m']/(c['x_endpoints_px'][1]-c['x_endpoints_px'][0])
        sy=c['span_m']/(c['y_endpoints_px'][1]-c['y_endpoints_px'][0])
        cx=sum(c['x_endpoints_px'])/2
        cy=sum(c['y_endpoints_px'])/2
        ox=(index%3)*58
        oy=-(index//3)*62
        image=bpy.data.images.load(str(refs/c['image']),check_existing=True)
        ref=bpy.data.objects.new('TRACE_REF_'+f['id'],None)
        ref.empty_display_type='IMAGE'
        ref.data=image
        ref.empty_display_size=reference_display_size(c)
        ref.empty_image_offset=(-.5,-.5)
        ref.scale=(1,sy/sx,1)
        ref.location=(ox+(w/2-cx)*sx,oy+(cy-h/2)*sy,-.1)
        ref.empty_image_depth='BACK'
        ref.empty_image_side='DOUBLE_SIDED'
        ref['export_to_game']=False
        qcoll.objects.link(ref)
        paths=[([(ox+x,oy+y,0) for x,y in f['outline_m']],True)]
        for x,y in centres:
            paths.append(([(ox+x+.34*math.cos(j*math.tau/24),oy+y+.34*math.sin(j*math.tau/24),.02) for j in range(24)],True))
        curve('TRACE_'+f['id'],paths,qcoll,(.0,.35,.85,1),.032)
        label(qcoll,f"{f['id']} / {f['figure']} / {len(centres)} columns",(ox-16,oy+23,0),1.1)

    stacked['measurement_source']='docs/refactoring/yellow-crane-plan-traces.json'
    exploded['display_only']='Each layer translated vertically by index*5.5m; physical levels unchanged in metadata.'
    qa['calibration']='Provisional manual X/Y registration; +/-3 image pixels, no deskew.'
    report={'status':'structural_study_only','scene_names':names,'floors':metrics,'total_selected_plan_symbols':sum(r['selected_columns'] for r in metrics),'mesh_triangles_stacked':sum(r['triangles'] for r in metrics),'runtime_game_modified':False,'roof_model_complete':False,'export_to_game':False}
    report_path.write_text(json.dumps(report,indent=2)+'\n')
    bpy.context.window.scene=stacked
    for area in bpy.context.screen.areas:
        if area.type=='VIEW_3D':
            space=area.spaces.active
            space.shading.type='SOLID'
            space.shading.color_type='OBJECT'
            space.overlay.show_overlays=True
            space.overlay.show_floor=False
            space.overlay.show_axis_x=False
            space.overlay.show_axis_y=False
            space.region_3d.view_perspective='CAMERA'
            space.region_3d.view_camera_zoom=20
            for setting in ('light','show_shadows','show_cavity','cavity_type','curvature_ridge_factor','curvature_valley_factor','background_type'):
                setattr(space.shading,setting,getattr(stacked.display.shading,setting))
    bpy.ops.wm.save_as_mainfile(filepath=str(blend_path))
    return report
