"""Versioned, non-exportable exterior massing; never an as-built reconstruction.

Only reference elevations/selected axis positions are constrained. Roof curvature,
wall/opening profiles, thicknesses and finial are explicitly visual estimates.
Pure mesh helpers are testable outside Blender; build() is called via Blender MCP.
"""
import json
import math
import os
import tempfile
from pathlib import Path
from dimensioned_study import slab_mesh, column_mesh


def tier_outline(scale):
    # 2-2-4 SE corner, axis-mapped manual pixels; rotate, not a generic pagoda.
    quadrant = [(14,0),(14,-5),(14.6,-8.6),(12.1,-8),(11.9,-8.3),
                (12.6,-12.5),(8.5,-11.8),(8.1,-12.1),(8.5,-14.5),(5,-13.9)]
    points, tips = [], []
    for k in range(4):
        for i,(x,y) in enumerate(quadrant):
            for _ in range(k):
                x,y = y,-x
            if i in (2,5,8):
                tips.append(len(points))
            points.append((x*scale,y*scale))
    # Reverse clockwise source coordinates to outward-oriented CCW meshes.
    return list(reversed(points)), [len(points)-1-i for i in tips]


def roof_mesh(inner, outer, ridge_z, eave_z, tips, thickness=.18, steps=12, subdivisions=5, corner_rise=3.5):
    """Closed curved annular patch; inner/outer contours must be matching CCW.

    Curvature and corner rise are display estimates, NOT the uncalibrated eave
    development trace. Independent patches may overlap pending junction review.
    """
    if len(inner) != len(outer) or len(inner)<3 or thickness<=0 or steps<2 or subdivisions<1 or corner_rise<0:
        raise ValueError('Matching polygons, positive thickness and subdivisions required')
    vertices, faces = [], []
    n = len(outer)*subdivisions
    for j in range(steps+1):
        t = j/steps
        for i in range(len(outer)):
            nxt = (i+1)%len(outer)
            for q in range(subdivisions):
                s = q/subdivisions
                a = tuple(inner[i][k]*(1-s)+inner[nxt][k]*s for k in (0,1))
                b = tuple(outer[i][k]*(1-s)+outer[nxt][k]*s for k in (0,1))
                lift = ((1-s)**3 if i in tips else 0)+(s**3 if nxt in tips else 0)
                z = eave_z+(ridge_z-eave_z)*(1-t)**1.65 + corner_rise*lift*t**4
                vertices.append((a[0]*(1-t)+b[0]*t, a[1]*(1-t)+b[1]*t,z))
    count = len(vertices)
    vertices += [(x,y,z-thickness) for x,y,z in vertices]
    for j in range(steps):
        for i in range(n):
            a,b = j*n+i,j*n+(i+1)%n
            face = [a,a+n,b+n,b]
            faces.extend([face,[v+count for v in reversed(face)]])
    for i in range(n):
        a,b = i,(i+1)%n
        faces.append([a,b,b+count,a+count])
        a,b = steps*n+i,steps*n+(i+1)%n
        faces.append([b,a,a+count,b+count])
    return vertices,faces


def enclosure_height(elevation, schematic_top):
    """Ground enclosure stops below its canopy; upper stories retain v04 heights."""
    if elevation == 0:
        return 7.1
    return (40.6 if elevation == 32.6 else schematic_top) - elevation


def transition_band(outline, floor_z):
    """Batched box descriptors for estimated rectangular cells under the L2 slab.

    Plate 2-2-7 shows a divided band, not evidence of transparent glass. Opaque
    recessed cells preserve this uncertainty. Heights, divisions and sections
    are visual estimates. Outline must be a simple orthogonal floor perimeter.
    """
    if len(outline)<4 or not math.isfinite(floor_z) or any(not math.isfinite(v) for p in outline for v in p):
        raise ValueError('Finite orthogonal outline and floor level required')
    area=sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(outline,outline[1:]+outline[:1]))
    if abs(area)<1e-8:
        raise ValueError('Non-degenerate outline required')
    normals=[]
    for (ax,ay),(bx,by) in zip(outline,outline[1:]+outline[:1]):
        dx,dy=bx-ax,by-ay
        if (dx==0)==(dy==0):
            raise ValueError('Non-zero axis-aligned edges required')
        length=math.hypot(dx,dy)
        sign=1 if area>0 else -1
        normals.append((-dy/length*sign,dx/length*sign))
    # Offset both adjacent supporting lines; unlike scaling, this retains steps.
    inset=[(x+.12*(normals[i-1][0]+normals[i][0]),y+.12*(normals[i-1][1]+normals[i][1])) for i,(x,y) in enumerate(outline)]
    members=[]
    bottom,top=floor_z-1.65,floor_z-.85
    for i,((ax,ay),(bx,by)) in enumerate(zip(inset,inset[1:]+inset[:1])):
        length=math.hypot(bx-ax,by-ay)
        if length<.4:
            raise ValueError('Band edges must accommodate a framed cell')
        ux,uy=(bx-ax)/length,(by-ay)/length
        nx,ny=normals[i]
        def member(label,distance,z,width,height,material='wood',depth=.22,recess=0):
            center=(ax+ux*distance+nx*recess,ay+uy*distance+ny*recess,z)
            size=(width,depth,height) if ux else (depth,width,height)
            members.append((label,center,size,material))
        member('Transition_sill',length/2,bottom-.075,length+.22,.15,'stone')
        member('Transition_header',length/2,(top+floor_z-.15)/2,length+.22,floor_z-.15-top,'wall')
        bays=max(1,round(length/4))
        pitch=length/bays
        for j in range(bays+1):
            member('Transition_piers',j*pitch,(bottom+top)/2,.20,top-bottom,'wall')
        for j in range(bays):
            start=j*pitch+.10
            clear=pitch-.20
            for z in (bottom+.03,top-.03):
                member('Transition_frames',start+clear/2,z,clear,.06)
            cells=max(1,round(clear/.85))
            cell_pitch=clear/cells
            for k in range(1,cells):
                member('Transition_frames',start+k*cell_pitch,(bottom+top)/2,.055,top-bottom-.12)
            for k in range(cells):
                member('Transition_recess_cells',start+(k+.5)*cell_pitch,(bottom+top)/2,cell_pitch-.055,top-bottom-.12,'recess',depth=.04,recess=.07)
    return members


def component_volumes(vertices, faces):
    """Signed volume per connected shell; a large shell cannot hide an inverted one."""
    parent=list(range(len(vertices)))
    def find(i):
        while parent[i]!=i:
            parent[i]=parent[parent[i]]
            i=parent[i]
        return i
    for face in faces:
        for i in face[1:]:
            parent[find(i)]=find(face[0])
    volumes={}
    for face in faces:
        key=find(face[0])
        a=vertices[face[0]]
        value=0
        for j in range(1,len(face)-1):
            b,c=vertices[face[j]],vertices[face[j+1]]
            value+=(a[0]*(b[1]*c[2]-b[2]*c[1])+a[1]*(b[2]*c[0]-b[0]*c[2])+a[2]*(b[0]*c[1]-b[1]*c[0]))/6
        volumes[key]=volumes.get(key,0)+value
    return list(volumes.values())


def output_paths(root, version):
    directory = Path(root)/'.omx/references/yellow-crane'
    paths = (directory/f'exterior-whitebox-{version}.blend',directory/f'exterior-whitebox-{version}-validation.json')
    for p in paths:
        if p.exists():
            raise FileExistsError(f'Preserve existing exterior: {p}')
    return paths


def approach_steps(base_half=18.5, depth=.42):
    """Four six-step approaches, contacting base edge and top at -0.15 m."""
    steps=[]
    for side in range(4):
        for i in range(6):
            x,y=0,-(base_half+depth/2-.02+i*.4)
            for _ in range(side):
                x,y=-y,x
            height=1.10-i*.18
            size=(8,depth,height) if side%2==0 else (depth,8,height)
            steps.append(((x,y,-1.25+height/2),size))
    return steps


def save_checkpoint(blend, report, payload, save_blend):
    """Publish report only after successful save; never replace existing outputs."""
    if blend.exists() or report.exists():
        raise FileExistsError('Checkpoint already exists; use a new version')
    encoded=json.dumps(payload,indent=2)
    with tempfile.NamedTemporaryFile(mode='w',dir=report.parent,prefix='.whitebox-',suffix='.tmp',delete=False) as f:
        staging=Path(f.name)
        f.write(encoded)
    try:
        result=save_blend()
        if 'FINISHED' not in result or not blend.is_file():
            raise OSError('Blender save did not finish; preserve scene for retry')
        os.link(staging,report) # Atomic no-replace publication on the same filesystem.
    finally:
        staging.unlink(missing_ok=True)


def save_scene_checkpoint(scene, blend, report):
    import bpy
    payload=json.loads(scene['pending_report'])
    scene['checkpoint_state']='saved'
    try:
        save_checkpoint(blend,report,payload,lambda: bpy.ops.wm.save_as_mainfile(filepath=str(blend),compress=True))
    except Exception:
        scene['checkpoint_state']='save_failed'
        raise
    return {'scene':scene.name,'blend':str(blend),'report':str(report),'mesh_count':len(payload['meshes']),'triangles':payload['triangles']}


def build(root, version='01'):
    import bpy
    import bmesh
    from mathutils import Vector
    root = Path(root)
    blend,report = output_paths(root,version)
    name = f'HHL_Exterior_Whitebox_{version}'
    if name in bpy.data.scenes:
        existing=bpy.data.scenes[name]
        if existing.get('checkpoint_state')=='save_failed':
            return save_scene_checkpoint(existing,blend,report)
        raise RuntimeError('Version already present; preserve it and use a new version')
    data=json.loads((root/'docs/refactoring/yellow-crane-plan-traces.json').read_text())
    scene=bpy.data.scenes.new(name)
    scene.unit_settings.system='METRIC'
    scene['status']='M1_WIP_estimated_exterior_NOT_as_built_NOT_game_ready'
    scene['export_to_game']=False
    groups={}
    for key in ('Design_axis_constraints','Estimated_roofs','Estimated_enclosure','Estimated_details','Presentation'):
        c=bpy.data.collections.new(f'{version}_{key}')
        scene.collection.children.link(c)
        groups[key]=c
    materials={}
    for key,color in {'roof':(.57,.61,.64,1),'wall':(.78,.75,.68,1),'wood':(.43,.40,.35,1),'stone':(.66,.68,.66,1),'recess':(.24,.28,.30,1)}.items():
        m=bpy.data.materials.new(f'WB{version}_{key}')
        m.diffuse_color=color
        materials[key]=m

    def mesh(label, geometry, group='Estimated_details', material='stone'):
        vertices,faces=geometry
        m=bpy.data.meshes.new(label)
        m.from_pydata(vertices,[],faces)
        m.update()
        obj=bpy.data.objects.new(f'WB{version}_{label}',m)
        groups[group].objects.link(obj)
        obj.data.materials.append(materials[material])
        if group=='Estimated_roofs':
            for polygon in m.polygons:
                polygon.use_smooth = True
        obj['accuracy']='visual_estimate; not measured; not approved for game'
        obj['export_to_game']=False
        return obj

    batches={}
    def box(label,center,size,material='wood'):
        vertices,faces=batches.setdefault((label,material),([],[]))
        x,y,z=center; a,b,c=(v/2 for v in size); offset=len(vertices)
        vertices.extend([(x-a,y-b,z-c),(x+a,y-b,z-c),(x+a,y+b,z-c),(x-a,y+b,z-c),
                         (x-a,y-b,z+c),(x+a,y-b,z+c),(x+a,y+b,z+c),(x-a,y+b,z+c)])
        faces.extend([[offset+i for i in f] for f in [(3,2,1,0),(4,5,6,7),(0,1,5,4),(1,2,6,5),(2,3,7,6),(3,0,4,7)]])

    # Retain selected printed levels/grid; enclosure/roof profiles remain estimates.
    for floor in data['floors']:
        if floor['id']=='M1':
            continue # internal mezzanine is not a second exposed exterior storey
        z=floor['elevation_m']
        outline=floor['outline_m']
        mesh(f"{floor['id']}_floor",slab_mesh(outline,floor['holes'],z,.30),'Design_axis_constraints')
        centres=[(data['axes_x_m'][str(col)],data['axes_y_m'][row]) for row,cols in floor['column_axes_by_row'].items() for col in cols]
        h=7.1 if z==0 else 5.15
        obj=mesh(f"{floor['id']}_posts",column_mesh(centres,z,z+h,.24),'Design_axis_constraints','wood')
        obj['accuracy']='selected design XY/level; estimated height and diameter'
        # Frame/panel walls inset behind open corridors. Central bays stay open.
        half=11 if z==0 else (7 if z<32 else 5.9)
        for side in range(4):
            for x in range(-int(half)+1,int(half),2):
                def rotate(a,b):
                    for _ in range(side): a,b=-b,a
                    return a,b
                px,py=rotate(x,half)
                width=(1.65,.22) if side%2==0 else (.22,1.65)
                if abs(x)>1:
                    box('Wall_panels',(px,py,z+1.05),(*width,2.1),'wall')
                    for dx in (-.7,0,.7):
                        wx,wy=rotate(x+dx,half)
                        box('Window_mullions',(wx,wy,z+3.05),(.09,.09,1.8))
                box('Wall_lintels',(px,py,z+4.18),(*width,.35))
                ceiling = enclosure_height(z, floor['schematic_post_top_m'])
                box('Upper_enclosure_frieze',(px,py,z+(4.35+ceiling)/2),(*((2.04,.24) if side%2==0 else (.24,2.04)),ceiling-4.35),'wall')
        # Estimated post-head beam silhouettes; detailed dougong belongs to M2.
        for (ax,ay),(bx,by) in zip(outline,outline[1:]+outline[:1]):
            # Ground slab is larger than column envelope; avoid detached outside beams.
            if z>0:
                box('Post_head_beams',((ax+bx)/2,(ay+by)/2,z+5.0),(abs(bx-ax)+.35,abs(by-ay)+.35,.42))
        # Open perimeter rails on upper corridors, batched rather than hundreds of objects.
        if z>0:
            for (ax,ay),(bx,by) in zip(outline,outline[1:]+outline[:1]):
                length=math.hypot(bx-ax,by-ay)
                if length<.1: continue
                for level in (.25,1.05):
                    box('Balcony_rails',((ax+bx)/2,(ay+by)/2,z+level),(abs(bx-ax)+.14,abs(by-ay)+.14,.14),'stone')
                for j in range(int(length/.65)+1):
                    t=j/max(1,int(length/.65))
                    box('Balcony_balusters',(ax+(bx-ax)*t,ay+(by-ay)*t,z+.65),(.10,.10,.8),'stone')

    second_floor=next(f for f in data['floors'] if f['id']=='L2')
    for label,center,size,material in transition_band(second_floor['outline_m'],second_floor['elevation_m']):
        box(label,center,size,material)

    for label,scale,inner_ratio,ridge,eave in [
        ('Ground_canopy',1.29,.50,10.21,7.23),
        ('Second_canopy',1.03,.62,19.4,17.22),
        ('Third_canopy',1,.61,26,23.82),
        ('Fourth_canopy',1,.61,32.6,30.42),
        ('Fifth_lower_canopy',.96,.55,40.6,37.02)]:
        outer,tips=tier_outline(scale)
        inner=[(x*inner_ratio,y*inner_ratio) for x,y in outer]
        obj=mesh(label,roof_mesh(inner,outer,ridge,eave,tips),'Estimated_roofs','roof')
        obj['source']='2-2-4 corner topology transferred/scaled as estimate; 2-2-5 section elevations; L4 conflict retained in eave trace'
    outer=[(-12.3,-12.1),(12.3,-12.1),(12.3,12.1),(-12.3,12.1)]
    inner=[(-.9,-.9),(.9,-.9),(.9,.9),(-.9,.9)]
    mesh('Crown_four_hip_sectors',roof_mesh(inner,outer,46.2,39.22,[0,1,2,3]),'Estimated_roofs','roof')
    for k in range(4):
        outer=[(-6,5.9),(6,5.9),(6.3,12.1),(-6.3,12.1)]
        inner=[(-3.3,7.3),(3.3,7.3),(3.3,7.5),(-3.3,7.5)]
        def rotated(points):
            out=[]
            for x,y in points:
                for _ in range(k): x,y=-y,x
                out.append((x,y))
            return out
        mesh(f'Crown_independent_wing_{k}',roof_mesh(rotated(inner),rotated(outer),43.2,39.3,[2,3]),'Estimated_roofs','roof')
    box('Crown_seat',(0,0,46.12),(1.85,1.85,.25),'stone')
    for k in range(4):
        x,y=0,7.4
        for _ in range(k):x,y=-y,x
        size=(6.65,.35,.25) if k%2==0 else (.35,6.65,.25)
        box('Wing_ridge_caps',(x,y,43.2),size,'stone')
    # Finial silhouette only: radius/z samples are NOT survey dimensions.
    rings=[(46.0,.82),(46.3,.82),(46.5,.45),(46.7,.63),(47.1,.70),(47.4,.40),
           (47.6,.28),(47.9,.52),(48.2,.55),(48.5,.25),(48.7,.19),(49.0,.30),(49.3,.13),(49.65,.025)]
    v=[];f=[];sides=24
    for z,r in rings:
        v.extend((r*math.cos(i*2*math.pi/sides),r*math.sin(i*2*math.pi/sides),z) for i in range(sides))
    for j in range(len(rings)-1):
        for i in range(sides):
            a=j*sides+i;b=j*sides+(i+1)%sides
            f.append([a,b,b+sides,a+sides])
    f += [list(reversed(range(sides))),list(range((len(rings)-1)*sides,len(rings)*sides))]
    mesh('Finial_silhouette_PROXY',(v,f))
    box('Base',(0,0,-.7),(37,37,1.1),'stone')
    for center,size in approach_steps():
        box('Approach_steps',center,size,'stone')
    for (label,material),geometry in batches.items():
        mesh(label,geometry,'Estimated_enclosure',material)
    # Neutral studio, not a cinematic render masking massing errors.
    scene.render.engine='BLENDER_WORKBENCH'
    scene.display.shading.light='STUDIO'
    scene.display.shading.studiolight_rotate_z=.35
    scene.display.shading.color_type='MATERIAL'
    scene.display.shading.show_shadows=True
    scene.display.shading.show_cavity=True
    scene.display.shading.cavity_type='BOTH'
    scene.display.shading.background_type='WORLD'
    scene.world=bpy.data.worlds.new(f'WB{version}_world')
    scene.world.color=(.18,.18,.18)
    scene.render.resolution_x=1400;scene.render.resolution_y=1600
    scene.render.resolution_percentage=100
    cameras={}
    for label,location in [('Front',(0,-110,24)),('Oblique',(80,-115,72))]:
        c=bpy.data.cameras.new(f'WB{version}_{label}')
        o=bpy.data.objects.new(c.name,c);groups['Presentation'].objects.link(o)
        o.location=location;o.rotation_euler=(Vector((0,0,24))-o.location).to_track_quat('-Z','Y').to_euler()
        c.type='ORTHO';c.ortho_scale=65 if label=='Oblique' else 59;c.lens=50
        cameras[label]=o
    scene.camera=cameras['Oblique']
    bpy.context.window.scene=scene
    for area in bpy.context.screen.areas:
        if area.type=='VIEW_3D':
            area.spaces.active.region_3d.view_perspective='CAMERA'
            area.spaces.active.region_3d.view_camera_zoom=5
            area.spaces.active.region_3d.view_camera_offset=(0,0)
            area.spaces.active.overlay.show_overlays=False
    results=[]
    for obj in scene.objects:
        if obj.type!='MESH':continue
        bm=bmesh.new();bm.from_mesh(obj.data)
        volumes=component_volumes([tuple(v.co) for v in obj.data.vertices],[list(p.vertices) for p in obj.data.polygons])
        results.append({'object':obj.name,'non_manifold_edges':sum(not e.is_manifold for e in bm.edges),'inconsistent_winding_edges':sum(e.is_manifold and not e.is_contiguous for e in bm.edges),'signed_volume':bm.calc_volume(signed=True),'component_count':len(volumes),'min_component_volume':min(volumes),'triangles':sum(len(p.vertices)-2 for p in obj.data.polygons)})
        bm.free()
    if any(r['non_manifold_edges'] or r['inconsistent_winding_edges'] or r['signed_volume']<=0 or r['min_component_volume']<=0 for r in results):
        raise ValueError(f'Invalid whitebox meshes: {results}')
    scene['pending_report']=json.dumps({'status':'M1_in_progress_not_visually_accepted','source':'design plates, NOT as-built','scene':name,'meshes':results,'triangles':sum(r['triangles'] for r in results),'limitations':['Roof intersections untrimmed','Transferred lower-tier outline estimates','No material/UV/detail/game acceptance','Wall openings and finial proxy dimensions','L2 transition cells are opaque visual proxies, not verified window construction']},indent=2)
    return save_scene_checkpoint(scene,blend,report)
