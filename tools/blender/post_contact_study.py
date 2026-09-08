"""Read-only contact diagnostics for convex vertical post prisms and roof shells.

Separate from the production square support fitter: no geometry is moved or fitted.
Positive-area projected overlaps are rejected before coverage is classified.
Coverage is never treated as structural acceptance.
"""
import math
from collections import Counter


def cross(a, b, p):
    return (b[0]-a[0])*(p[1]-a[1])-(b[1]-a[1])*(p[0]-a[0])


def _area(poly):
    return abs(sum(cross(poly[0],p,q) for p,q in zip(poly,poly[1:]+poly[:1])))/2 if len(poly)>2 else 0.


def _clip(poly, edges):
    for a,b in edges:
        clipped=[]
        for p,q in zip(poly,poly[1:]+poly[:1]):
            dp,dq=cross(a,b,p),cross(a,b,q)
            if dp>=0: clipped.append(p)
            if (dp>=0)!=(dq>=0):
                t=dp/(dp-dq)
                clipped.append(tuple(p[k]+t*(q[k]-p[k]) for k in range(len(p))))
        poly=clipped
    return poly


def polygon_probe(triangles, footprint):
    """Exact planar-triangle minimum on a strictly convex polygon intersection."""
    footprint=list(footprint)
    if len(footprint)<3 or any(len(p)!=2 or not all(math.isfinite(x) for x in p) for p in footprint):
        raise ValueError('Finite convex XY footprint required')
    # Translation-stable signed area, and every vertex must lie inside every edge.
    signed=sum(cross(footprint[0],a,b) for a,b in zip(footprint,footprint[1:]+footprint[:1]))/2
    if signed<0: footprint.reverse()
    edges=list(zip(footprint,footprint[1:]+footprint[:1]))
    if abs(signed)<1e-12 or len(set(map(tuple,footprint)))!=len(footprint) or any(
            cross(a,b,p)<-1e-10 for a,b in edges for p in footprint):
        raise ValueError('Nondegenerate convex footprint required')
    area=0.; minimum=None; source=None; projected_parts=[]
    for index,triangle in enumerate(triangles):
        if len(triangle)!=3 or any(len(p)!=3 or not all(math.isfinite(v) for v in p) for p in triangle):
            raise ValueError('Finite XYZ triangles required')
        if cross(*triangle)>=0: continue
        poly=_clip(list(triangle),edges)
        part=_area(poly)
        if part<1e-12: continue
        # Triangle projections are clockwise; reverse to clip with CCW half-planes.
        projected=list(reversed([(p[0],p[1]) for p in poly]))
        for other in projected_parts:
            if _area(_clip(projected,list(zip(other,other[1:]+other[:1]))))>1e-10:
                raise ValueError('Overlapping underside projection violates single-layer contract')
        projected_parts.append(projected)
        area+=part
        point=min(poly,key=lambda p:(p[2],p[0],p[1]))
        if minimum is None or point[2]<minimum[2]: minimum=tuple(point); source=index
    target=abs(signed)
    if area>target+max(1e-8,target*1e-6):
        raise ValueError('Overlapping underside projection violates single-layer contract')
    coverage='none' if minimum is None else ('full' if math.isclose(area,target,rel_tol=1e-6,abs_tol=1e-8) else 'partial')
    return {'cap_m':minimum[2] if minimum else None,'minimum_point':minimum,
            'triangle_index':source,'projected_area_m2':area,'footprint_area_m2':target,'coverage':coverage}


def vertical_prisms(vertices, faces):
    """Extract connected actual flat-cap prisms; reject slants/unsupported topology.

    A component must have matching XY rings, exactly two caps and quad side faces.
    This deliberately does not approximate arbitrary posts by a convex hull.
    """
    if any(len(v)!=3 or not all(math.isfinite(x) for x in v) for v in vertices):
        raise ValueError('Finite prism vertices required')
    adjacency=[set() for _ in vertices]
    for face in faces:
        if len(face)<3 or len(set(face))!=len(face) or any(i<0 or i>=len(vertices) for i in face): raise ValueError('Invalid face')
        for a,b in zip(face,face[1:]+face[:1]): adjacency[a].add(b); adjacency[b].add(a)
    remaining=set(range(len(vertices))); posts=[]
    while remaining:
        todo=[min(remaining)]; component=set()
        while todo:
            i=todo.pop()
            if i in component: continue
            component.add(i); todo.extend(adjacency[i]-component)
        remaining-=component
        bottom=min(vertices[i][2] for i in component); top=max(vertices[i][2] for i in component)
        lower={i for i in component if abs(vertices[i][2]-bottom)<1e-6}
        upper={i for i in component if abs(vertices[i][2]-top)<1e-6}
        caps=[f for f in faces if set(f)==upper]
        bottom_caps=[f for f in faces if set(f)==lower]
        sides=[f for f in faces if set(f)<=component and set(f)!=upper and set(f)!=lower]
        if top-bottom<=1e-6 or lower|upper!=component or len(lower)!=len(upper) or len(caps)!=1 or len(bottom_caps)!=1 or len(sides)!=len(upper) or any(len(f)!=4 for f in sides):
            raise ValueError('Expected flat-capped vertical prism')
        low_xy=sorted(tuple(vertices[i][:2]) for i in lower)
        high_xy=sorted(tuple(vertices[i][:2]) for i in upper)
        if any(math.dist(a,b)>1e-6 for a,b in zip(low_xy,high_xy)):
            raise ValueError('Slanted/tapered post unsupported')
        # Match the actual two rings, then require precisely their extrusion faces.
        matching={i:min(lower,key=lambda j:math.dist(vertices[i][:2],vertices[j][:2])) for i in upper}
        if len(set(matching.values()))!=len(upper): raise ValueError('Ambiguous ring correspondence')
        ring=caps[0]
        def cycle(face):
            return min(tuple(face[i:]+face[:i]) for i in range(len(face)))
        expected_sides=[cycle([b,a,matching[a],matching[b]]) for a,b in zip(ring,ring[1:]+ring[:1])]
        if Counter(map(cycle,sides))!=Counter(expected_sides) or cycle(bottom_caps[0])!=cycle([matching[i] for i in reversed(ring)]):
            raise ValueError('Faces do not form a closed consistently wound vertical prism')
        footprint=[tuple(vertices[i][:2]) for i in caps[0]]
        polygon_probe([],footprint)  # Validate convex cap rather than silently hulling it.
        center=tuple(sum(p[k] for p in footprint)/len(footprint) for k in (0,1))
        posts.append({'center':center,'footprint':footprint,'bottom_m':bottom,'top_m':top})
    return posts


def run_preflight(root):
    """Pure-source rehearsal, NOT readback of Blender's saved/evaluated mesh.

    Explicitly fans quads; actual Blender tessellation and float32 vertices must
    still be checked separately. Keeps production geometry/functions unchanged.
    """
    import hashlib
    import json
    from pathlib import Path
    from dimensioned_study import column_mesh
    from exterior_whitebox import canopy_roof_mesh, roof_mesh, tier_outline
    root=Path(root)
    data=json.loads((root/'docs/refactoring/yellow-crane-plan-traces.json').read_text())
    rows=[]
    # Mirrored production parameters are explicit fixtures, not measured geometry.
    fixtures=[('L1','Ground_canopy',1.29,.50,10.21,7.23),
              ('L3','Third_canopy',1.,.61,26.,23.82)]
    for floor_id,label,scale,ratio,ridge,eave in fixtures:
        floor=next(f for f in data['floors'] if f['id']==floor_id)
        centres=[(data['axes_x_m'][str(c)],data['axes_y_m'][r])
                 for r,cols in floor['column_axes_by_row'].items() for c in cols]
        bottom=floor['elevation_m']; top=bottom+(7.1 if bottom==0 else 5.15)
        posts=vertical_prisms(*column_mesh(centres,bottom,top,.24))
        outer,tips=tier_outline(scale); inner=[(x*ratio,y*ratio) for x,y in outer]
        versions={}
        for version,geometry in [('13',roof_mesh(inner,outer,ridge,eave,tips)),
                                 ('14',canopy_roof_mesh(label,inner,outer,ridge,eave,tips))]:
            vertices,faces=geometry
            triangles=[tuple(vertices[k] for k in (p[0],p[j],p[j+1])) for p in faces for j in range(1,len(p)-1)]
            # Broadphase only discards disjoint AABBs; footprint clipping decides coverage.
            versions[version]=[(t,tuple(min(p[k] for p in t) for k in (0,1)),
                                tuple(max(p[k] for p in t) for k in (0,1))) for t in triangles if cross(*t)<0]
        for post in posts:
            footprint=post['footprint']; low=[min(p[k] for p in footprint) for k in (0,1)]; high=[max(p[k] for p in footprint) for k in (0,1)]
            row={'floor':floor_id,'center':post['center'],'post_bottom_m':bottom,'post_top_m':top,'footprint_sides':len(footprint)}
            for version,items in versions.items():
                nearby=[t for t,a,b in items if all(b[k]>=low[k] and a[k]<=high[k] for k in (0,1))]
                result=polygon_probe(nearby,footprint)
                result['vertical_gap_m']=None if result['cap_m'] is None else result['cap_m']-top
                # Cap below top is a diagnostic flag, not a solid intersection proof.
                row['v'+version]=result
            rows.append(row)
    covered=[r for r in rows if r['v14']['coverage']=='full']
    negative=[r for r in covered if r['v14']['vertical_gap_m'] < -3e-6]
    return {'recorded_on':'2026-09-08','status':'pure_source_preflight_only_runtime_readback_blocked',
            'actual_blender_readback':False,'solid_intersection_verified':False,'export_to_game':False,
            'geometry_modified':False,'fixtures':fixtures,'rows':rows,
            'summary':{'posts':len(rows),'full_coverage':len(covered),
                       'partial_coverage':sum(r['v14']['coverage']=='partial' for r in rows),
                       'no_coverage':sum(r['v14']['coverage']=='none' for r in rows),
                       'negative_full_footprints':len(negative),
                       'minimum_vertical_gap_m':min(r['v14']['vertical_gap_m'] for r in covered),
                       'maximum_negative_gap_worsening_m':max(r['v13']['vertical_gap_m']-r['v14']['vertical_gap_m'] for r in negative)},
            'source_sha256':{p:hashlib.sha256((root/p).read_bytes()).hexdigest() for p in
                ('tools/blender/exterior_whitebox.py','tools/blender/dimensioned_study.py',
                 'tools/blender/post_contact_study.py','docs/refactoring/yellow-crane-plan-traces.json')},
            'limits':['Double-precision generated vertices and fan triangulation, NOT actual Blender loop triangles or saved-mesh readback.',
                      'Polygon coverage rejects pairwise projected overlap; height minima alone do not establish closed-solid intersection.',
                      'Production parameter fixtures must be rechecked after generator changes. Post heights/diameter remain estimates.',
                      'No model correction, visual re-render, architecture acceptance, game integration or performance claim.']}
