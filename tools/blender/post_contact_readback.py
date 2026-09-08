"""Read-only contact investigation on extracted stored/evaluated mesh arrays.

Fallback reports inspect BOTH quad diagonals explicitly, never label them as
Blender loop triangles. No bpy import unless capture_scene is called in Blender.
"""
import hashlib
import json
from collections import Counter
from post_contact_study import cross, polygon_probe, vertical_prisms
from solid_witness import winding_number


def capture_scene(scene, version):
    """Capture actual mesh coordinates/loop triangles, without saving or editing.

    Reject evaluated modifiers/parents instead of silently inspecting a different
    shape. calc_loop_triangles populates a runtime cache, not mesh geometry.
    """
    result={}
    for suffix in ('L1_posts','L3_posts','Ground_canopy','Third_canopy'):
        obj=scene.objects[f'WB{version}_{suffix}']
        if obj.parent or obj.modifiers or obj.constraints or obj.animation_data or obj.data.shape_keys or obj.data.animation_data:
            raise ValueError('Unmodified static mesh required for bounded contact audit')
        obj.data.calc_loop_triangles()
        result[obj.name]={'name':obj.name,
                          'vertices':[list(obj.matrix_world @ v.co) for v in obj.data.vertices],
                          'faces':[list(p.vertices) for p in obj.data.polygons],
                          'loop_triangles':[list(t.vertices) for t in obj.data.loop_triangles]}
    return result


def triangulate(mesh, diagonal):
    """Return explicit quad re-triangulation; never impersonates runtime tessellation."""
    if diagonal not in (0,1): raise ValueError('Quad diagonal must be 0 or 1')
    if any(len(f)!=4 for f in mesh['faces']): raise ValueError('Expected all-quad roof shell')
    vertices=mesh['vertices']; result=[]
    for face in mesh['faces']:
        f=face[diagonal:]+face[:diagonal]
        result.extend(tuple(tuple(vertices[i]) for i in t) for t in
                      ((f[0],f[1],f[2]),(f[0],f[2],f[3])))
    return result


def _near(triangles, footprint):
    low=[min(p[k] for p in footprint) for k in (0,1)]
    high=[max(p[k] for p in footprint) for k in (0,1)]
    return [t for t in triangles if all(max(p[k] for p in t)>=low[k] and
            min(p[k] for p in t)<=high[k] for k in (0,1))]


def _vertical_interval(triangles, xy):
    lower=[]; upper=[]
    for t in triangles:
        det=cross(*t)
        if abs(det)<1e-12: continue
        weights=[cross(t[1],t[2],xy)/det,cross(t[2],t[0],xy)/det,cross(t[0],t[1],xy)/det]
        if min(weights)<-1e-10: continue
        z=sum(w*p[2] for w,p in zip(weights,t))
        (lower if det<0 else upper).append(z)
    return (max(lower),min(upper)) if lower and upper else None


def _post_triangles(post):
    # vertical_prisms already verified matching caps and every actual side edge.
    fp=post['footprint']; lower=[(*p,post['bottom_m']) for p in fp]; upper=[(*p,post['top_m']) for p in fp]
    triangles=[]
    for i in range(len(fp)):
        j=(i+1)%len(fp)
        triangles.extend([(lower[i],lower[j],upper[j]),(lower[i],upper[j],upper[i])])
    for i in range(1,len(fp)-1):
        triangles.extend([(lower[0],lower[i+1],lower[i]),(upper[0],upper[i],upper[i+1])])
    return triangles


def interior_witness(post, roofs, seeds):
    """Seek ONE shared interior point, using independent solid angles for confirmation.

    Sampling failure is inconclusive, never evidence of clearance. roofs contains
    both full re-triangulated shells (or actual runtime triangles in a later audit).
    """
    near=[_near(ts,post['footprint']) for ts in roofs]
    for seed in seeds:
        for inward in (.01,.05,.2,.5,.9):
            xy=tuple(seed[k]*(1-inward)+post['center'][k]*inward for k in (0,1))
            intervals=[_vertical_interval(ts,xy) for ts in near]
            if any(interval is None for interval in intervals): continue
            lo=max(post['bottom_m'],*(iv[0] for iv in intervals))
            hi=min(post['top_m'],*(iv[1] for iv in intervals))
            if hi-lo<1e-3: continue
            point=(*xy,(lo+hi)/2)
            try:
                roof_winding=[winding_number(ts,point) for ts in roofs]
                post_winding=winding_number(_post_triangles(post),point)
            except ValueError:
                continue  # Degenerate/boundary/unsupported shell: not a passing witness.
            if abs(post_winding-1)<1e-6 and all(abs(w-1)<1e-6 for w in roof_winding):
                return {'point_m':point,'roof_winding_numbers':roof_winding,'post_winding_number':post_winding,
                        'common_vertical_interval_m':[lo,hi],
                        'interval_half_width_m':(hi-lo)/2}
    return None


def analyze_stored_meshes(meshes):
    """Both-diagonal fallback only; deliberately does NOT consume loop_triangles."""
    rows=[]; metadata={}
    for floor,roof_name in [('L1','Ground_canopy'),('L3','Third_canopy')]:
        versions={}; posts_by_version={}
        for version in ('13','14'):
            post_mesh=meshes[f'WB{version}_{floor}_posts']; roof=meshes[f'WB{version}_{roof_name}']
            posts_by_version[version]=vertical_prisms(post_mesh['vertices'],post_mesh['faces'])
            versions[version]=[triangulate(roof,diagonal) for diagonal in (0,1)]
            for mesh in (post_mesh,roof):
                raw=json.dumps([mesh['vertices'],mesh['faces']],separators=(',',':')).encode()
                metadata[mesh['name']]={'vertices':len(mesh['vertices']),'polygons':len(mesh['faces']),
                                       'coordinates_faces_sha256':hashlib.sha256(raw).hexdigest()}
        if posts_by_version['13']!=posts_by_version['14']:
            raise ValueError('Post geometry changed; paired regression needs fresh review')
        for index,post in enumerate(posts_by_version['14']):
            row={'floor':floor,'post_index':index,'center_m':post['center'],
                 'top_m':post['top_m'],'footprint_sides':len(post['footprint'])}
            for version in ('13','14'):
                probes=[]
                for ts in versions[version]:
                    probe=polygon_probe(_near(ts,post['footprint']),post['footprint'])
                    probe['vertical_gap_m']=probe['cap_m']-post['top_m'] if probe['cap_m'] is not None else None
                    probes.append(probe)
                negative=all(p['coverage']=='full' and p['vertical_gap_m']<-3e-6 for p in probes)
                witness=interior_witness(post,versions[version],[p['minimum_point'] for p in probes]) if negative else None
                row['v'+version]={'diagonals':probes,'negative_on_both':negative,'interior_witness':witness}
            rows.append(row)
    summary={'posts':len(rows)}
    for version in ('13','14'):
        key='v'+version
        summary[key]={
            'coverage_by_diagonal':[dict(Counter(r[key]['diagonals'][d]['coverage'] for r in rows)) for d in (0,1)],
            'negative_on_both':sum(r[key]['negative_on_both'] for r in rows),
            'witnesses_inside_both_retriangulations':sum(r[key]['interior_witness'] is not None for r in rows),
            'minimum_gap_by_diagonal_m':[min(r[key]['diagonals'][d]['vertical_gap_m'] for r in rows
                                          if r[key]['diagonals'][d]['vertical_gap_m'] is not None) for d in (0,1)]}
    summary['maximum_existing_negative_worsening_by_diagonal_m']=[
        max(r['v13']['diagonals'][d]['vertical_gap_m']-r['v14']['diagonals'][d]['vertical_gap_m']
            for r in rows if r['v13']['negative_on_both']) for d in (0,1)]
    return {'mesh_metadata':metadata,'summary':summary,'rows':rows}
