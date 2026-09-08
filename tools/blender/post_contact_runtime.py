"""Bounded contact audit using captured Blender loop triangles, never a fallback.

Shared interior witnesses are local evidence, not self-intersection, structural,
architectural fidelity, or whole-scene acceptance. Partial/no coverage is unknown.
"""
import hashlib
import json
import math
from collections import Counter
from post_contact_study import polygon_probe, vertical_prisms
from post_contact_readback import _near, interior_witness
from solid_witness import winding_number


def runtime_triangles(mesh):
    """Validate per-polygon oriented tessellation, then retain its exact indices."""
    vertices=mesh['vertices']; faces=mesh['faces']; loops=mesh.get('loop_triangles')
    if not loops or len(loops)!=sum(len(f)-2 for f in faces):
        raise ValueError('Complete captured loop triangles required')
    if any(len(v)!=3 or not all(math.isfinite(x) for x in v) for v in vertices):
        raise ValueError('Finite XYZ vertices required')
    memberships=[set() for _ in vertices]
    for i,face in enumerate(faces):
        if len(face)<3 or len(set(face))!=len(face) or any(type(j) is not int or not 0<=j<len(vertices) for j in face):
            raise ValueError('Valid polygon indices required')
        for j in face: memberships[j].add(i)
    groups=[[] for _ in faces]
    for tri in loops:
        if len(tri)!=3 or len(set(tri))!=3 or any(type(j) is not int or not 0<=j<len(vertices) for j in tri):
            raise ValueError('Valid captured triangle indices required')
        owners=set.intersection(*(memberships[j] for j in tri))
        if len(owners)!=1: raise ValueError('Triangle must belong to exactly one source polygon')
        groups[owners.pop()].append(tuple(tri))
    for face,triangles in zip(faces,groups):
        if len(triangles)!=len(face)-2: raise ValueError('Incomplete polygon tessellation')
        edges=Counter((p,q) for tri in triangles for p,q in zip(tri,tri[1:]+tri[:1]))
        boundary=Counter(zip(face,face[1:]+face[:1]))
        for (p,q),count in edges.items():
            if count!=1 or count-edges[(q,p)]!=boundary[(p,q)]-boundary[(q,p)]:
                raise ValueError('Tessellation must preserve oriented polygon boundary')
        if any(edges[e]!=1 or edges[(e[1],e[0])]!=0 for e in boundary):
            raise ValueError('Missing polygon boundary')
    return [tuple(tuple(vertices[j]) for j in tri) for tri in loops]


def analyze_runtime_meshes(meshes):
    rows=[]; metadata={}
    for floor,roof_name in [('L1','Ground_canopy'),('L3','Third_canopy')]:
        posts={}; roofs={}; post_triangles={}
        for version in ('13','14'):
            post_mesh=meshes[f'WB{version}_{floor}_posts']; roof=meshes[f'WB{version}_{roof_name}']
            posts[version]=vertical_prisms(post_mesh['vertices'],post_mesh['faces'])
            post_triangles[version]=runtime_triangles(post_mesh)
            roofs[version]=runtime_triangles(roof)
            for suffix,mesh in [(floor+'_posts',post_mesh),(roof_name,roof)]:
                raw=json.dumps([mesh['vertices'],mesh['faces']],separators=(',',':')).encode()
                loops=json.dumps(mesh['loop_triangles'],separators=(',',':')).encode()
                metadata[f'WB{version}_{suffix}']={
                    'vertices':len(mesh['vertices']),'polygons':len(mesh['faces']),
                    'loop_triangles':len(mesh['loop_triangles']),
                    'coordinates_faces_sha256':hashlib.sha256(raw).hexdigest(),
                    'loop_triangles_sha256':hashlib.sha256(loops).hexdigest()}
        if posts['13']!=posts['14']:
            raise ValueError('Post geometry changed; paired regression needs fresh review')
        for index,post in enumerate(posts['14']):
            row={'floor':floor,'post_index':index,'center_m':post['center'],'top_m':post['top_m']}
            for version in ('13','14'):
                probe=polygon_probe(_near(roofs[version],post['footprint']),post['footprint'])
                gap=probe['cap_m']-post['top_m'] if probe['cap_m'] is not None else None
                negative=probe['coverage']=='full' and gap<-3e-6
                witness=interior_witness(post,[roofs[version]],[probe['minimum_point']]) if negative else None
                if witness:
                    try:
                        # Use the captured post tessellation too, not just the discovery prism.
                        actual=winding_number(post_triangles[version],witness['point_m'])
                    except ValueError:
                        witness=None
                    else:
                        if abs(actual-1)<1e-6: witness['actual_post_winding_number']=actual
                        else: witness=None
                row['v'+version]={'probe':probe,'vertical_gap_m':gap,
                                 'negative_full_coverage':negative,'interior_witness':witness}
            rows.append(row)
    summary={'posts':len(rows)}
    for version in ('v13','v14'):
        full=[r[version]['vertical_gap_m'] for r in rows if r[version]['probe']['coverage']=='full']
        summary[version]={
            'coverage':dict(Counter(r[version]['probe']['coverage'] for r in rows)),
            'negative_full_coverage':sum(r[version]['negative_full_coverage'] for r in rows),
            'runtime_interior_witnesses':sum(r[version]['interior_witness'] is not None for r in rows),
            'minimum_full_coverage_gap_m':min(full,default=None)}
    summary['maximum_existing_negative_worsening_m']=max([0.]+[
        r['v13']['vertical_gap_m']-r['v14']['vertical_gap_m'] for r in rows
        if r['v13']['negative_full_coverage'] and r['v14']['probe']['coverage']=='full'])
    return {'mesh_metadata':metadata,'summary':summary,'rows':rows}


def audit_capture(capture, stored):
    """Cross-check capture provenance; caller obtains capture through Blender MCP."""
    if capture['geometry_modified'] is not False or capture['geometry_before']!=capture['geometry_after']:
        raise ValueError('Read-only capture geometry mismatch')
    if capture['source_blend_sha256']['14']!=stored['source_blend_sha256']:
        raise ValueError('Capture source differs from reviewed blend')
    report=analyze_runtime_meshes(capture['meshes'])
    for name,meta in report['mesh_metadata'].items():
        if meta['coordinates_faces_sha256']!=stored['mesh_metadata'][name]['coordinates_faces_sha256']:
            raise ValueError('Runtime coordinates/faces differ from reviewed stored mesh')
    # Provenance is input data; it must never replace computed rows or verdicts.
    report.update({k:capture[k] for k in ('blender_version','active_scene',
                   'source_blend_sha256','geometry_before','geometry_after','geometry_modified')})
    report.update({
        'actual_blender_readback':True,'runtime_loop_triangles_verified':True,
        'bounded_runtime_interior_witnesses_verified':all(
            s['negative_full_coverage']==s['runtime_interior_witnesses']
            for s in (report['summary']['v13'],report['summary']['v14'])),
        'solid_intersection_verified':False,'export_to_game':False,
        'stored_coordinates_faces_match':True,
        'limitations':[
            'Only L1/L3 posts and their Ground/Third canopies in v13/v14 were audited.',
            'Positive winding witnesses establish local shared interior, conditional on non-self-intersecting shells; no global self-intersection or Boolean-volume certification.',
            'Partial/no roof coverage remains unaccepted; no architectural fidelity or M1 visual acceptance.',
            'Private runtime capture arrays are not shipped; hashes bind this numeric report to that local capture.',
            'Saved blend file hashes identify unchanged disk sources; runtime scene arrays were independently matched to stored extraction.']})
    return report


if __name__=='__main__':
    import argparse
    from pathlib import Path
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('capture',type=Path)
    parser.add_argument('stored_report',type=Path)
    parser.add_argument('output',type=Path)
    args=parser.parse_args()
    result=audit_capture(json.loads(args.capture.read_text()),json.loads(args.stored_report.read_text()))
    result['capture_sha256']=hashlib.sha256(args.capture.read_bytes()).hexdigest()
    result['stored_report_sha256']=hashlib.sha256(args.stored_report.read_bytes()).hexdigest()
    result['helper_sha256']={name:hashlib.sha256(Path(__file__).with_name(name).read_bytes()).hexdigest()
                             for name in ('post_contact_runtime.py','post_contact_readback.py',
                                          'post_contact_study.py','solid_witness.py','corner_clearance_study.py')}
    args.output.write_text(json.dumps(result,indent=2,ensure_ascii=False)+'\n')
    print(json.dumps(result['summary'],indent=2))
