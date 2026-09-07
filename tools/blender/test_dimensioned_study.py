"""Pure-Python geometry regressions; does not launch Blender or touch scenes."""
import json
import unittest
import tempfile
from collections import Counter
from pathlib import Path
from dimensioned_study import inside, slab_mesh, column_mesh, validate_trace, study_output_paths, reference_display_size

ROOT = Path(__file__).resolve().parents[2]
TRACE = json.loads((ROOT / 'docs/refactoring/yellow-crane-plan-traces.json').read_text())

class GeometryTests(unittest.TestCase):
    def test_portrait_reference_preserves_axis_scale(self):
        c = TRACE['calibrations']['2-2-6']
        w,h = c['image_size_px']
        self.assertGreater(h, w)
        actual_width = reference_display_size(c) * w / max(w,h)
        required_width = w*c['span_m']/(c['x_endpoints_px'][1]-c['x_endpoints_px'][0])
        self.assertAlmostEqual(actual_width, required_width)

    def test_trace_contract(self):
        validate_trace(TRACE)
        self.assertEqual([f['column_count'] for f in TRACE['floors']], [72,52,44,44,44,36])

    def test_wall_rounds_are_not_columns(self):
        h = TRACE['floors'][0]['column_axes_by_row']['H']
        self.assertFalse(set(h) & {5,6,7,8})

    def test_unknown_roof_stays_absent(self):
        self.assertFalse(TRACE['export_to_game'])
        self.assertEqual(TRACE['display_proxies']['roof'], 'absent_pending_unfolded_curve_mapping')

    def test_slab_is_closed_with_open_atrium(self):
        for f in TRACE['floors']:
            with self.subTest(floor=f['id']):
                verts, faces = slab_mesh(f['outline_m'], f['holes'], f['elevation_m'], .24)
                edges = Counter(tuple(sorted((a,b))) for face in faces for a,b in zip(face,face[1:]+face[:1]))
                self.assertTrue(edges)
                self.assertEqual(set(edges.values()), {2})
                self.assertAlmostEqual(max(v[2] for v in verts), f['elevation_m'])
                self.assertAlmostEqual(min(v[2] for v in verts), f['elevation_m']-.24)
                for face in faces:
                    self.assertEqual(len(set(face)), len(face))
                    if all(abs(verts[j][2]-f['elevation_m']) < 1e-8 for j in face):
                        x=sum(verts[j][0] for j in face)/len(face)
                        y=sum(verts[j][1] for j in face)/len(face)
                        for h in f['holes']:
                            a,b,c,d=h['rect_m']
                            self.assertFalse(a < x < c and b < y < d)

    def test_rectangle_with_hole_area(self):
        verts,faces=slab_mesh([[-2,-2],[2,-2],[2,2],[-2,2]], [{'rect_m':[-1,-1,1,1]}], 0, .2)
        area=0
        for face in faces:
            if all(verts[j][2]==0 for j in face):
                pts=[verts[j] for j in face]
                area += sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(pts,pts[1:]+pts[:1]))/2
        self.assertAlmostEqual(area,12)

    def test_cylinder_closed_and_outward(self):
        verts, faces=column_mesh([(0,0)],0,6,.3)
        edges=Counter(tuple(sorted((a,b))) for f in faces for a,b in zip(f,f[1:]+f[:1]))
        self.assertEqual(set(edges.values()),{2})
        directed = Counter((a,b) for f in faces for a,b in zip(f,f[1:]+f[:1]))
        self.assertTrue(all(count == directed[(b,a)] for (a,b),count in directed.items()))
        # Triangulate face fans and sum signed tetrahedral volumes.
        volume = 0
        for f in faces:
            a = verts[f[0]]
            for i in range(1, len(f)-1):
                b,c = verts[f[i]], verts[f[i+1]]
                volume += (a[0]*(b[1]*c[2]-b[2]*c[1])
                           + a[1]*(b[2]*c[0]-b[0]*c[2])
                           + a[2]*(b[0]*c[1]-b[1]*c[0]))/6
        self.assertGreater(volume, 0)
        self.assertEqual(len(verts),32)
        self.assertEqual(max(v[2] for v in verts),6)

    def test_invalid_inputs_rejected(self):
        with self.assertRaises(ValueError):
            slab_mesh([[0,0],[1,1],[2,0]],[],0,.2)
        with self.assertRaises(ValueError):
            column_mesh([(0,0)],6,0,.3)

    def test_existing_outputs_are_preserved_without_blender(self):
        for filename in ('dimensioned-structure-study.blend', 'structure-study-validation.json'):
            with self.subTest(filename=filename), tempfile.TemporaryDirectory() as folder:
                root = Path(folder)
                refs = root / '.omx/references/yellow-crane'
                refs.mkdir(parents=True)
                output = refs / filename
                output.write_text('existing user work')
                with self.assertRaises(FileExistsError):
                    study_output_paths(root)
                self.assertEqual(output.read_text(), 'existing user work')

    def test_unused_output_paths_allowed(self):
        with tempfile.TemporaryDirectory() as folder:
            blend, report = study_output_paths(Path(folder))
            self.assertEqual(blend.name, 'dimensioned-structure-study.blend')
            self.assertEqual(report.name, 'structure-study-validation.json')
            self.assertFalse(blend.exists())

    def test_point_in_polygon(self):
        p=[[-1,-1],[1,-1],[1,1],[-1,1]]
        self.assertTrue(inside(0,0,p))
        self.assertFalse(inside(2,0,p))

if __name__ == '__main__':
    unittest.main()
