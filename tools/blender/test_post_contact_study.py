"""Regression boundaries for actual polygon footprints, not square fitting."""
import math
import unittest
from post_contact_study import polygon_probe, vertical_prisms
from dimensioned_study import column_mesh


def plane():
    return [((-2,-2,6),(-2,2,10),(2,2,14)),
            ((-2,-2,6),(2,2,14),(2,-2,10))]  # downward, z=10+x+y


class PostContactTests(unittest.TestCase):
    def test_square_false_positive_disappears_on_actual_round_polygon(self):
        footprint=[(.24*math.cos(i*math.tau/16),.24*math.sin(i*math.tau/16)) for i in range(16)]
        probe=polygon_probe(plane(),footprint)
        from top_support_whitebox import underside_probe
        self.assertLess(underside_probe(plane(),(0,0),.48)['cap_m'],9.6)
        self.assertGreater(probe['cap_m'],9.6)
        self.assertAlmostEqual(probe['cap_m'],10-.24*math.sqrt(2))
        self.assertEqual(probe['coverage'],'full')

    def test_true_negative_and_reversed_footprint(self):
        diamond=[(-.24,0),(0,-.24),(.24,0),(0,.24)]
        a=polygon_probe(plane(),diamond)
        b=polygon_probe(plane(),list(reversed(diamond)))
        self.assertAlmostEqual(a['cap_m'],9.76)
        self.assertLess(a['cap_m']-9.8,0)
        self.assertAlmostEqual(a['cap_m'],b['cap_m'])
        self.assertAlmostEqual(a['projected_area_m2'],.24**2*2)

    def test_partial_missing_and_upward_are_not_clearance_passes(self):
        footprint=[(-1,-1),(1,-1),(1,1),(-1,1)]
        self.assertEqual(polygon_probe(plane()[:1],footprint)['coverage'],'partial')
        for triangles in [[],[tuple(reversed(t)) for t in plane()]]:
            p=polygon_probe(triangles,footprint)
            self.assertEqual(p['coverage'],'none')
            self.assertIsNone(p['cap_m'])

    def test_interior_minimum_and_input_preservation(self):
        rim=[(-2,-2,10),(-2,2,10),(2,2,10),(2,-2,10)]
        triangles=[(rim[i],rim[(i+1)%4],(0,0,8)) for i in range(4)]
        before=repr(triangles)
        self.assertEqual(polygon_probe(iter(triangles),[(-1,-1),(1,-1),(1,1),(-1,1)])['minimum_point'],(0,0,8))
        self.assertEqual(repr(triangles),before)

    def test_rejects_invalid_and_overlapping_coverage(self):
        for f in [[],[(0,0),(0,0),(1,1)],[(0,0),(1,0),(0.1,.1),(0,1)],[(0,0),(math.nan,0),(0,1)]]:
            with self.assertRaises(ValueError): polygon_probe(plane(),f)
        with self.assertRaises(ValueError):
            polygon_probe(plane()*2,[(-1,-1),(1,-1),(1,1),(-1,1)])

    def test_duplicate_half_cannot_mask_a_hole(self):
        with self.assertRaises(ValueError):
            polygon_probe([plane()[0],plane()[0]],[(-1,-1),(1,-1),(1,1),(-1,1)])

    def test_malformed_prism_topology_is_rejected(self):
        vertices,faces=column_mesh([(0,0)],20,25,.24)
        for mode in ('duplicate_side','diagonal_side','crossed_bottom'):
            malformed=[list(f) for f in faces]
            if mode=='duplicate_side': malformed[0]=malformed[1][:]
            elif mode=='diagonal_side': malformed[0]=[0,2,18,16]
            else: malformed[-2][0],malformed[-2][2]=malformed[-2][2],malformed[-2][0]
            with self.subTest(mode=mode), self.assertRaises(ValueError):
                vertical_prisms(vertices,malformed)

    def test_actual_prism_caps_extracted_and_slanted_posts_rejected(self):
        vertices,faces=column_mesh([(2,3),(4,5)],20,25,.24)
        posts=vertical_prisms(vertices,faces)
        self.assertEqual(len(posts),2)
        self.assertEqual([p['center'] for p in posts],[(2,3),(4,5)])
        self.assertTrue(all(len(p['footprint'])==16 and p['bottom_m']==20 and p['top_m']==25 for p in posts))
        vertices[16]=(vertices[16][0]+.01,vertices[16][1],25)
        with self.assertRaises(ValueError): vertical_prisms(vertices,faces)
