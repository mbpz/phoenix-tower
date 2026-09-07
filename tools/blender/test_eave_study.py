import json
import unittest
from pathlib import Path
from eave_study import display_points

DATA = json.loads((Path(__file__).resolve().parents[2]/'docs/refactoring/yellow-crane-eave-traces.json').read_text())


class EaveTests(unittest.TestCase):
    def test_source_pixels_roundtrip_without_metric_assumption(self):
        display = DATA['display']
        a,b,c,d = display['crop_bounds_px']
        scale = display['pixels_per_display_unit']
        points = display_points(DATA)
        self.assertEqual(len(points), 14)
        for (x,y,z), (px,py) in zip(points, DATA['asymmetric_lower_curve']['points_px']):
            self.assertAlmostEqual(x*scale+(a+c)/2, px)
            self.assertAlmostEqual((b+d)/2-y*scale, py)
        self.assertIsNone(DATA['asymmetric_lower_curve']['metric_points'])
        self.assertIsNone(DATA['asymmetric_lower_curve']['roof_plan_mapping'])
        self.assertFalse(DATA['export_to_game'])

    def test_conflicts_are_not_silently_corrected(self):
        conflicts = DATA['unresolved_conflicts']
        self.assertEqual(len(conflicts), 2)
        self.assertTrue(all(c['resolution'] is None for c in conflicts))
        self.assertEqual(DATA['symmetric_elevations_m'][3]['lower_right'], 32)
        self.assertEqual(DATA['asymmetric_elevations_m'][0]['lower_right'], 17.2)

    def test_invalid_display_scale_rejected(self):
        data = dict(DATA, display=dict(DATA['display'], pixels_per_display_unit=0))
        with self.assertRaises(ValueError):
            display_points(data)


if __name__ == '__main__':
    unittest.main()
