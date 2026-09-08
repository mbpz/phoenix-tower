"""Runtime evidence must consume, validate, and retain captured tessellation."""
import copy
import unittest
from dimensioned_study import column_mesh
from test_post_contact_readback import box
from post_contact_runtime import runtime_triangles, analyze_runtime_meshes, audit_capture


def captured(mesh):
    mesh=copy.deepcopy(mesh)
    mesh['loop_triangles']=[(f[0],f[i],f[i+1]) for f in mesh['faces'] for i in range(1,len(f)-1)]
    return mesh


def scene_pair(height=.8):
    meshes={}
    for version in ('13','14'):
        for floor,roof in [('L1','Ground_canopy'),('L3','Third_canopy')]:
            v,f=column_mesh([(0,0)],0,1,.24)
            meshes[f'WB{version}_{floor}_posts']=captured({'vertices':v,'faces':f})
            meshes[f'WB{version}_{roof}']=captured(box(height))
    return meshes


class RuntimeContactTests(unittest.TestCase):
    def test_uses_captured_diagonal_not_fallback(self):
        mesh=captured(box())
        mesh['loop_triangles'][:2]=[(2,1,0),(2,0,3)]
        self.assertEqual(runtime_triangles(mesh)[0],tuple(tuple(mesh['vertices'][i]) for i in (2,1,0)))

    def test_missing_invalid_reversed_or_duplicate_triangles_rejected(self):
        for mutate in (
                lambda m:m.pop('loop_triangles'),
                lambda m:m['loop_triangles'].pop(),
                lambda m:m['loop_triangles'].__setitem__(0,(-1,2,3)),
                lambda m:m['loop_triangles'].__setitem__(0,(3,1,2)),
                lambda m:m['loop_triangles'].__setitem__(0,m['loop_triangles'][1])):
            mesh=captured(box()); mutate(mesh)
            with self.assertRaises(ValueError): runtime_triangles(mesh)

    def test_shared_witness_checks_actual_post_triangles(self):
        report=analyze_runtime_meshes(scene_pair())
        self.assertEqual(report['summary']['posts'],2)
        for version in ('v13','v14'):
            self.assertEqual(report['summary'][version]['negative_full_coverage'],2)
            self.assertEqual(report['summary'][version]['runtime_interior_witnesses'],2)
            for row in report['rows']:
                self.assertAlmostEqual(row[version]['interior_witness']['actual_post_winding_number'],1.)

    def test_touch_separation_and_no_coverage_are_not_intersections(self):
        for height in (1.,1.1):
            report=analyze_runtime_meshes(scene_pair(height))
            self.assertEqual(report['summary']['v14']['runtime_interior_witnesses'],0)
            self.assertEqual(report['summary']['maximum_existing_negative_worsening_m'],0)
        meshes=scene_pair()
        for n,m in meshes.items():
            if n.endswith('canopy'): m['vertices']=[(x+10,y,z) for x,y,z in m['vertices']]
        report=analyze_runtime_meshes(meshes)
        self.assertEqual(report['summary']['v14']['coverage'],{'none':2})
        self.assertIsNone(report['summary']['v14']['minimum_full_coverage_gap_m'])

    def test_changed_posts_rejected(self):
        meshes=scene_pair()
        meshes['WB14_L1_posts']['vertices']=[(x,y,z+1) for x,y,z in meshes['WB14_L1_posts']['vertices']]
        with self.assertRaisesRegex(ValueError,'Post geometry changed'): analyze_runtime_meshes(meshes)

    def test_capture_provenance_and_geometry_guards(self):
        meshes=scene_pair()
        stored=analyze_runtime_meshes(meshes)
        stored['source_blend_sha256']='reviewed-source'
        capture={'meshes':meshes,'blender_version':'test-fixture','active_scene':'test',
                 'geometry_modified':False,'geometry_before':{'14':'hash'},
                 'geometry_after':{'14':'hash'},'source_blend_sha256':{'14':'reviewed-source'}}
        report=audit_capture(capture,stored)
        self.assertTrue(report['runtime_loop_triangles_verified'])
        self.assertFalse(report['solid_intersection_verified'])
        injected=copy.deepcopy(capture)
        injected.update(summary={'posts':0},rows=[],mesh_metadata={})
        self.assertEqual(audit_capture(injected,stored),report)
        for key,value in [('geometry_modified',True),('geometry_after',{'14':'changed'}),
                          ('source_blend_sha256',{'14':'different'})]:
            broken=copy.deepcopy(capture); broken[key]=value
            with self.assertRaises(ValueError): audit_capture(broken,stored)
        stored['mesh_metadata']['WB14_L1_posts']['coordinates_faces_sha256']='different'
        with self.assertRaisesRegex(ValueError,'coordinates/faces'): audit_capture(capture,stored)


if __name__=='__main__': unittest.main()
