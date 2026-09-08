import copy
import json
import unittest
from pathlib import Path
from l3_section_study import source_datums, section_segments, post_readout

ROOT = Path(__file__).resolve().parents[2]


class SectionStudyTests(unittest.TestCase):
    def setUp(self):
        self.data = json.loads((ROOT / 'docs/refactoring/yellow-crane-l3-section-constraints.json').read_text())
        self.detail = json.loads((ROOT / 'docs/refactoring/yellow-crane-eave-traces.json').read_text())

    def test_printed_chain_closes_and_matches_independent_detail(self):
        result = source_datums(self.data, self.detail)
        self.assertEqual(result['stations_m'], [26.,25.4,24.56,23.5,23.,22.2])
        self.assertAlmostEqual(result['detail_drop_m'], 1.58)
        self.assertFalse(result['column_top_verified'])
        self.assertFalse(result['transfer_to_exterior'])

    def test_corrupt_chain_or_semantic_promotion_rejected(self):
        for change in ('chain', 'detail', 'column_top_verified', 'transfer_to_exterior',
                       'inner_ring_surface_verified', 'plan_correspondence_verified'):
            data = copy.deepcopy(self.data)
            if change == 'chain': data['section']['descending_chain_mm'][0] += 1
            elif change == 'detail': data['detail']['upper_right_m'] += .001
            else: data[change] = True
            with self.assertRaises(ValueError): source_datums(data, self.detail)

    def test_compensating_chain_edits_cannot_leave_stale_post_readout(self):
        self.data['section']['descending_chain_mm'][1] += 100
        self.data['section']['descending_chain_mm'][2] -= 100
        datums = source_datums(self.data, self.detail)
        result = post_readout(datums, 24.55)
        self.assertAlmostEqual(result['support_station_m'], 24.46)
        self.assertAlmostEqual(result['support_station_minus_estimated_post_top_m'], -.09)
        self.assertIn('24.460 m', result['label'])
        self.assertIn('-90.0 mm', result['label'])

    def test_slice_uses_triangle_intersection_not_height_fit(self):
        triangles = [[(-1,0,0),(1,0,0),(0,2,2)]]
        self.assertEqual(section_segments(triangles, 0, .5, 1.5), [((.5,.5),(1.5,1.5))])
        self.assertEqual(section_segments(triangles, 2, 0, 2), [])

    def test_reversed_winding_edge_and_tangent(self):
        t = [(-1,0,0),(1,0,0),(0,2,2)]
        self.assertEqual(section_segments([t],0,0,2),section_segments([t[::-1]],0,0,2))
        self.assertEqual(section_segments([[(0,0,0),(1,1,0),(1,0,1)]],0,-1,2),[])
        self.assertEqual(section_segments([[(0,0,0),(0,2,2),(1,0,0)]],0,0,2),[((0.,0.),(2.,2.))])

    def test_coplanar_and_nonfinite_inputs_are_not_silently_accepted(self):
        with self.assertRaisesRegex(ValueError,'Coplanar'):
            section_segments([[(0,0,0),(0,2,0),(0,0,2)]],0,0,2)
        for x,lo,hi in [(float('nan'),0,2),(0,2,1)]:
            with self.assertRaises(ValueError): section_segments([],x,lo,hi)
        with self.assertRaises(ValueError):
            section_segments([[(0,0,0),(1,float('inf'),0),(1,0,2)]],0,0,2)


if __name__ == '__main__': unittest.main()
