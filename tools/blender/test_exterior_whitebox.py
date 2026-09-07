"""Geometry regressions run without Blender or third-party dependencies."""
import math
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from exterior_whitebox import roof_mesh, tier_outline, output_paths, approach_steps, save_checkpoint, component_volumes


class ExteriorGeometryTests(unittest.TestCase):
    def assert_closed(self, vertices, faces):
        edges = Counter()
        directed = Counter()
        volume = 0
        for face in faces:
            for a, b in zip(face, face[1:] + face[:1]):
                edges[tuple(sorted((a, b)))] += 1
                directed[(a,b)] += 1
            a = vertices[face[0]]
            for j in range(1, len(face)-1):
                b, c = vertices[face[j]], vertices[face[j+1]]
                volume += (a[0]*(b[1]*c[2]-b[2]*c[1]) + a[1]*(b[2]*c[0]-b[0]*c[2]) + a[2]*(b[0]*c[1]-b[1]*c[0]))/6
        self.assertTrue(all(n == 2 for n in edges.values()))
        self.assertTrue(all(directed[(a,b)]==1 and directed[(b,a)]==1 for a,b in edges))
        self.assertGreater(volume, 0)
        self.assertTrue(all(v>0 for v in component_volumes(vertices,faces)))
        self.assertTrue(all(math.isfinite(n) for v in vertices for n in v))

    def test_stepped_outline_retains_twelve_tips(self):
        outline, tips = tier_outline(1)
        self.assertEqual(len(tips), 12)
        self.assertEqual(len(outline), len(set(outline)))
        self.assertEqual(len(outline), 40)

    def test_curved_roof_is_closed_and_outward(self):
        outline, tips = tier_outline(1)
        inner = [(x*.63, y*.63) for x,y in outline]
        self.assert_closed(*roof_mesh(inner, outline, 26, 23.82, tips))

    def test_top_roof_and_wing_are_closed(self):
        outer = [(-12,-12),(12,-12),(12,12),(-12,12)]
        inner = [(x*.07,y*.07) for x,y in outer]
        self.assert_closed(*roof_mesh(inner, outer, 46.2, 39.22, [0,1,2,3]))
        self.assert_closed(*roof_mesh([(-3.3,7.3),(3.3,7.3),(3.3,7.5),(-3.3,7.5)], [(-6,5.9),(6,5.9),(6.3,12.1),(-6.3,12.1)],43.2,39.3,[2,3]))

    def test_inward_component_not_hidden_by_positive_total(self):
        from dimensioned_study import column_mesh
        a,fa=column_mesh([(0,0)],0,3,1)
        b,fb=column_mesh([(4,0)],0,1,.3)
        faces=fa+[[len(a)+i for i in reversed(f)] for f in fb]
        volumes=component_volumes(a+b,faces)
        self.assertGreater(sum(volumes),0)
        self.assertLess(min(volumes),0)
        with self.assertRaises(AssertionError):
            self.assert_closed(a+b,faces)

    def test_reversed_face_is_detected(self):
        outline,tips=tier_outline(1)
        vertices,faces=roof_mesh([(x*.6,y*.6) for x,y in outline],outline,26,23.82,tips)
        faces[0].reverse()
        with self.assertRaises(AssertionError):
            self.assert_closed(vertices,faces)

    def test_steps_contact_platform_and_each_other(self):
        steps=approach_steps()
        self.assertLessEqual(abs(steps[0][0][1])-steps[0][1][1]/2,18.5)
        self.assertAlmostEqual(steps[0][0][2]+steps[0][1][2]/2,-.15)
        for (a,sa),(b,sb) in zip(steps[:6],steps[1:6]):
            self.assertLessEqual(abs(b[1])-sb[1]/2,abs(a[1])+sa[1]/2)

    def test_save_failure_does_not_publish_report(self):
        with tempfile.TemporaryDirectory() as d:
            blend,report=Path(d)/'a.blend',Path(d)/'a.json'
            def fail():
                raise OSError('injected disk failure')
            with self.assertRaises(OSError):
                save_checkpoint(blend,report,{'ok':True},fail)
            self.assertFalse(report.exists())
            def succeed():
                blend.touch()
                return {'FINISHED'}
            save_checkpoint(blend,report,{'ok':True},succeed)
            self.assertTrue(report.exists())

    def test_existing_report_preserved(self):
        with tempfile.TemporaryDirectory() as d:
            blend,report=output_paths(Path(d),'01')
            report.parent.mkdir(parents=True)
            report.write_text('preserve')
            with self.assertRaises(FileExistsError):
                output_paths(Path(d),'01')
            self.assertEqual(report.read_text(),'preserve')

    def test_invalid_roof_rejected(self):
        with self.assertRaises(ValueError):
            roof_mesh([], [], 1, 0, [])
        with self.assertRaises(ValueError):
            roof_mesh([(0,0)]*4, [(0,0)]*4, 1, 0, [], thickness=0)

    def test_existing_outputs_preserved(self):
        with tempfile.TemporaryDirectory() as d:
            blend, report = output_paths(Path(d), '01')
            blend.parent.mkdir(parents=True)
            blend.touch()
            with self.assertRaises(FileExistsError):
                output_paths(Path(d), '01')


if __name__ == '__main__':
    unittest.main()
