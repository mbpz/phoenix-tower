"""Geometry regressions run without Blender or third-party dependencies."""
import math
import tempfile
import unittest
from collections import Counter
from pathlib import Path
from exterior_whitebox import roof_mesh, tier_outline, output_paths, approach_steps, save_checkpoint, component_volumes, transition_band, enclosure_height


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

    def test_transition_band_is_bounded_and_fourfold_symmetric(self):
        import json
        data=json.loads((Path(__file__).resolve().parents[2]/'docs/refactoring/yellow-crane-plan-traces.json').read_text())
        outline=next(f['outline_m'] for f in data['floors'] if f['id']=='L2')
        members=transition_band(outline,12.3)
        def key(label,center,size,material):
            return label,tuple(round(v,6) for v in center),tuple(round(v,6) for v in size),material
        actual=Counter(key(*m) for m in members)
        rotated=Counter(key(label,(-y,x,z),(sy,sx,sz),material) for label,(x,y,z),(sx,sy,sz),material in members)
        self.assertEqual(actual,rotated)
        self.assertGreater(len(members),0)
        for _,(x,y,z),(sx,sy,sz),_ in members:
            self.assertTrue(all(math.isfinite(v) and v>0 for v in (sx,sy,sz)))
            self.assertGreaterEqual(z-sz/2,10.5-1e-9)
            self.assertLessEqual(z+sz/2,12.15+1e-9)
            self.assertLessEqual(abs(x)+sx/2,12+1e-9)
            self.assertLessEqual(abs(y)+sy/2,12+1e-9)

    def test_transition_cells_are_recessed_not_a_solid_front_wall(self):
        members=transition_band([(-6,-6),(-6,6),(6,6),(6,-6)],12.3)
        cells=[m for m in members if m[0]=='Transition_recess_cells' and m[1][1]<-5]
        self.assertGreater(len(cells),4)
        for _,(x,y,z),(sx,sy,sz),material in cells:
            self.assertGreater(y-sy/2,-5.88) # behind frame centre plane
            self.assertLess(sz,.8)
            self.assertLess(sx,1.1)
            self.assertEqual(material,'recess')
        # No full-height blank member may run the entire facade in the cell band.
        for label,(_,_,z),(sx,sy,sz),_ in members:
            if abs(z-11.05)<.1 and label!='Transition_recess_cells':
                self.assertLess(min(sx,sy),.25)
                self.assertLess(max(sx,sy),12)

    def test_transition_band_rejects_invalid_contours(self):
        for outline in ([],[(0,0)]*4,[(0,0),(1,1),(0,2)],[(0,0),(0,1),(float('nan'),1),(1,0)]):
            with self.assertRaises(ValueError):
                transition_band(outline,12.3)
        with self.assertRaises(ValueError):
            transition_band([(-6,-6),(-6,6),(6,6),(6,-6)],float('nan'))

    def test_transition_band_accepts_either_winding_and_moves_with_floor(self):
        outline=[(-6,-6),(-6,6),(6,6),(6,-6)]
        def normalized(members,offset=0):
            return Counter((label,tuple(round(v,5) for v in (x,y,z-offset)),tuple(round(v,5) for v in size),material) for label,(x,y,z),size,material in members)
        base=normalized(transition_band(outline,12.3))
        self.assertEqual(base,normalized(transition_band(list(reversed(outline)),12.3)))
        self.assertEqual(base,normalized(transition_band(outline,13.3),1))

    def test_only_ground_enclosure_ceiling_changes(self):
        self.assertEqual(enclosure_height(0,6),7.1)
        self.assertAlmostEqual(enclosure_height(12.3,19.4),7.1)
        self.assertAlmostEqual(enclosure_height(19.4,26),6.6)
        self.assertAlmostEqual(enclosure_height(26,32.6),6.6)
        self.assertAlmostEqual(enclosure_height(32.6,40.6),8)

    def test_existing_outputs_preserved(self):
        with tempfile.TemporaryDirectory() as d:
            blend, report = output_paths(Path(d), '01')
            blend.parent.mkdir(parents=True)
            blend.touch()
            with self.assertRaises(FileExistsError):
                output_paths(Path(d), '01')


if __name__ == '__main__':
    unittest.main()
