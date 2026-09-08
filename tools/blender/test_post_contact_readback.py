"""Stored-mesh fallback remains distinct from evaluated Blender acceptance."""
import unittest
from dimensioned_study import column_mesh
from post_contact_study import vertical_prisms
from post_contact_readback import triangulate, interior_witness, analyze_stored_meshes


def box(z=.8):
    v=[(-1,-1,z),(1,-1,z),(1,1,z),(-1,1,z),(-1,-1,z+.4),(1,-1,z+.4),(1,1,z+.4),(-1,1,z+.4)]
    return {'vertices':v,'faces':[[3,2,1,0],[4,5,6,7],[0,1,5,4],[1,2,6,5],[2,3,7,6],[3,0,4,7]]}


class ContactReadbackTests(unittest.TestCase):
    def test_two_diagonals_have_independent_shared_interior_witness(self):
        post=vertical_prisms(*column_mesh([(0,0)],0,1,.24))[0]
        roofs=[triangulate(box(),d) for d in (0,1)]
        witness=interior_witness(post,roofs,[(.24,0,.8)])
        self.assertIsNotNone(witness)
        self.assertAlmostEqual(witness['point_m'][2],.9)
        self.assertAlmostEqual(witness['interval_half_width_m'],.1)
        self.assertTrue(all(abs(w-1)<1e-6 for w in witness['roof_winding_numbers']))
        self.assertAlmostEqual(witness['post_winding_number'],1.)

    def test_no_witness_for_touch_or_separation(self):
        post=vertical_prisms(*column_mesh([(0,0)],0,1,.24))[0]
        for height in (1.,1.1):
            roofs=[triangulate(box(height),d) for d in (0,1)]
            self.assertIsNone(interior_witness(post,roofs,[(.24,0,height)]))

    def test_partial_coverage_and_bad_shell_cannot_fabricate_witness(self):
        post=vertical_prisms(*column_mesh([(0,0)],0,1,.24))[0]
        mesh=box(); mesh['faces']=mesh['faces'][:-1]
        self.assertIsNone(interior_witness(post,[triangulate(mesh,0)],[(.24,0,.8)]))
        with self.assertRaises(ValueError): triangulate(box(),2)
        mesh['faces'][0]=[0,1,2]
        with self.assertRaises(ValueError): triangulate(mesh,0)

    def test_paired_summary_and_post_change_guard(self):
        meshes={}
        for version in ('13','14'):
            for floor,roof in [('L1','Ground_canopy'),('L3','Third_canopy')]:
                name=f'WB{version}_{floor}_posts'; v,f=column_mesh([(0,0)],0,1,.24)
                meshes[name]={'name':name,'vertices':v,'faces':f}
                name=f'WB{version}_{roof}'; meshes[name]={'name':name,**box()}
        report=analyze_stored_meshes(meshes)
        self.assertEqual(report['summary']['posts'],2)
        for version in ('v13','v14'):
            self.assertEqual(report['summary'][version]['negative_on_both'],2)
            self.assertEqual(report['summary'][version]['witnesses_inside_both_retriangulations'],2)
        meshes['WB14_L1_posts']['vertices'],meshes['WB14_L1_posts']['faces']=column_mesh([(0,0)],0,1.1,.24)
        with self.assertRaises(ValueError): analyze_stored_meshes(meshes)


if __name__=='__main__': unittest.main()
