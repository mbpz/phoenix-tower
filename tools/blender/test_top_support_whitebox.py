"""Geometry limits for estimated supports, not architectural certification."""
import math
import unittest
from top_support_whitebox import underside_cap, support_members, perimeter_centres


def plane():
    # Downward winding; z = 10 + x.
    return [((-2,-2,8),(-2,2,8),(2,2,12)),
            ((-2,-2,8),(2,2,12),(2,-2,12))]


class SupportTests(unittest.TestCase):
    def test_cap_uses_entire_footprint_not_center(self):
        self.assertAlmostEqual(underside_cap(plane(), (0,0), 1.04), 9.48)

    def test_minimum_inside_footprint_is_not_lost(self):
        rim=[(-2,-2,10),(-2,2,10),(2,2,10),(2,-2,10)]
        triangles=[(rim[i],rim[(i+1)%4],(0,0,8)) for i in range(4)]
        self.assertAlmostEqual(underside_cap(triangles,(0,0),1),8)

    def test_missing_coverage_rejected(self):
        for triangles in ([], plane()[:1]):
            with self.assertRaises(ValueError): underside_cap(triangles,(0,0),1)

    def test_upward_surface_is_not_an_underside(self):
        with self.assertRaises(ValueError):
            underside_cap([tuple(reversed(t)) for t in plane()],(0,0),1)

    def test_invalid_inputs_rejected(self):
        for center,width in [((0,0),0),((0,0),-1),((math.nan,0),1),((0,0),math.inf)]:
            with self.assertRaises(ValueError): underside_cap(plane(),center,width)
        with self.assertRaises(ValueError):
            underside_cap([((0,0,math.nan),(0,1,1),(1,0,1))],(0,0),1)
        for bottom in (math.nan,10,9.47):
            with self.assertRaises(ValueError): support_members(plane(),[(0,0)],bottom)

    def test_members_touch_and_stay_below_roof(self):
        members=support_members(plane(),[(0,0)],8)
        self.assertEqual(len(members),4)
        previous=8
        for center,size in members:
            self.assertAlmostEqual(center[2]-size[2]/2,previous)
            previous=center[2]+size[2]/2
            self.assertLessEqual(previous,10-size[0]/2)
        self.assertAlmostEqual(previous,9.46)
        self.assertEqual([s[0] for c,s in members],[.48,.64,.84,1.04])

    def test_only_unique_perimeter_columns(self):
        import json
        from pathlib import Path
        data=json.loads((Path(__file__).resolve().parents[2]/'docs/refactoring/yellow-crane-plan-traces.json').read_text())
        floor=next(f for f in data['floors'] if f['id']=='L5')
        centres=[(data['axes_x_m'][str(c)],data['axes_y_m'][r]) for r,cols in floor['column_axes_by_row'].items() for c in cols]
        selected=perimeter_centres(centres,floor['outline_m'])
        self.assertEqual(len(selected),20)
        self.assertEqual(len(set(selected)),20)
        self.assertTrue(all(abs(x)==9 or abs(y)==9 for x,y in selected))
        self.assertEqual(perimeter_centres(centres+centres,floor['outline_m']),selected)

class WitnessTests(unittest.TestCase):
    def test_witness_is_on_clipped_triangle_and_preserves_scalar_api(self):
        from top_support_whitebox import underside_probe
        result = underside_probe(plane(), (0, 0), 1.04)
        x, y, z = result['minimum_point']
        self.assertAlmostEqual(z, 9.48)
        self.assertAlmostEqual(z, 10 + x)
        self.assertLessEqual(abs(x), .52)
        self.assertLessEqual(abs(y), .52)
        self.assertIn(result['triangle_index'], (0, 1))
        self.assertAlmostEqual(result['projected_area'], 1.04**2)
        self.assertEqual(result['cap_m'], underside_cap(plane(), (0, 0), 1.04))

    def test_interior_minimum_is_a_witness_not_a_square_corner(self):
        from top_support_whitebox import underside_probe
        rim = [(-2,-2,10),(-2,2,10),(2,2,10),(2,-2,10)]
        triangles = [(rim[i],rim[(i+1)%4],(0,0,8)) for i in range(4)]
        self.assertEqual(underside_probe(triangles, (0,0), 1)['minimum_point'], (0,0,8))

    def test_probe_rejects_missing_coverage_and_does_not_mutate_input(self):
        from top_support_whitebox import underside_probe
        triangles = plane()
        before = repr(triangles)
        underside_probe(iter(triangles), (0,0), 1)
        self.assertEqual(repr(triangles), before)
        with self.assertRaises(ValueError):
            underside_probe(triangles[:1], (0,0), 1)


class ActualCanopyTests(unittest.TestCase):
    def test_corner_footprint_exposes_collision_center_ray_misses(self):
        from exterior_whitebox import roof_mesh, tier_outline
        outer,tips=tier_outline(.96)
        vertices,faces=roof_mesh([(x*.55,y*.55) for x,y in outer],outer,40.6,37.02,tips)
        triangles=[tuple(vertices[k] for k in (p[0],p[j],p[j+1])) for p in faces for j in range(1,len(p)-1)]
        # Pure fixture fans quads; Blender builds use its actual loop triangles.
        self.assertGreater(underside_cap(triangles,(9,9),.001),38)
        self.assertLess(underside_cap(triangles,(9,9),1.04),37.81)
        with self.assertRaises(ValueError): support_members(triangles,[(9,9)],37.75)

    def test_actual_canopy_study_keeps_all_four_corner_conflicts(self):
        from exterior_whitebox import roof_mesh, tier_outline
        from top_support_whitebox import support_study
        outer,tips=tier_outline(.96)
        vertices,faces=roof_mesh([(x*.55,y*.55) for x,y in outer],outer,40.6,37.02,tips)
        triangles=[tuple(vertices[k] for k in (p[0],p[j],p[j+1])) for p in faces for j in range(1,len(p)-1)]
        centres=[(x,y) for x in (-9,-5,-3,3,5,9) for y in (-9,9)]
        centres += [(x,y) for x in (-9,9) for y in (-5,-3,3,5)]
        study=support_study(triangles,centres,37.75)
        self.assertEqual(len(study['accepted']),16)
        self.assertEqual(len(study['members']),64)
        self.assertEqual({r['center'] for r in study['blocked']},{(-9,-9),(-9,9),(9,-9),(9,9)})

    def test_study_preserves_conflict_instead_of_shrinking_or_moving_columns(self):
        from top_support_whitebox import support_study
        result=support_study(plane(),[(0,0),(-1,0)],8.5)
        self.assertEqual(len(result['members']),4)
        self.assertEqual(result['accepted'],[(0,0)])
        self.assertEqual(result['blocked'][0]['center'],(-1,0))
        self.assertIn('Insufficient',result['blocked'][0]['reason'])


if __name__=='__main__': unittest.main()
