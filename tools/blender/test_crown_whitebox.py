"""Plan partition and shared-shell regressions, independent of Blender."""
import math
import unittest
from collections import Counter
from crown_whitebox import crown_mesh
import test_exterior_whitebox


def cross(a,b,c):
    return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])


def intersection_area(a,b):
    """Convex triangle clipping: detects positive-area overlap, not shared edges."""
    polygon=list(a)
    for p,q in zip(b,b[1:]+b[:1]):
        output=[]
        for s,t in zip(polygon,polygon[1:]+polygon[:1]):
            ds,dt=cross(p,q,s),cross(p,q,t)
            if (ds>=0)!=(dt>=0):
                u=ds/(ds-dt)
                output.append(tuple(s[i]+u*(t[i]-s[i]) for i in (0,1)))
            if dt>=0:output.append(t)
        polygon=output
        if not polygon:return 0
    return abs(sum(p[0]*q[1]-p[1]*q[0] for p,q in zip(polygon,polygon[1:]+polygon[:1])))/2


class CrownGeometryTests(unittest.TestCase):
    def test_shell_is_closed_outward_and_single_component(self):
        vertices,faces,regions=crown_mesh(subdivisions=4)
        test_exterior_whitebox.ExteriorGeometryTests().assert_closed(vertices,faces)
        from exterior_whitebox import component_volumes
        self.assertEqual(len(component_volumes(vertices,faces)),1)
        self.assertEqual(len(regions),len(faces))
        self.assertEqual(set(regions),{0,1,2,3,4})

    def test_plan_has_no_overlaps_and_covers_outer_square_minus_aperture(self):
        vertices,faces,_=crown_mesh(subdivisions=1)
        n=len(vertices)//2
        triangles=[[vertices[i][:2] for i in f] for f in faces if max(f)<n]
        area=sum(cross(*t)/2 for t in triangles)
        self.assertAlmostEqual(area,24.2**2-1.8**2,places=7)
        self.assertTrue(all(cross(*t)>0 for t in triangles))
        for i,a in enumerate(triangles):
            for b in triangles[i+1:]:
                self.assertLess(intersection_area(a,b),1e-8)

    def test_refined_top_has_one_height_per_plan_point_and_shared_regions(self):
        v,f,r=crown_mesh(subdivisions=6)
        n=len(v)//2
        self.assertEqual(len({(round(x,8),round(y,8)) for x,y,z in v[:n]}),n)
        edges={}
        for face,region in zip(f,r):
            if max(face)>=n:continue
            for a,b in zip(face,face[1:]+face[:1]):
                edges.setdefault(tuple(sorted((a,b))),set()).add(region)
        for wing in range(1,5):
            self.assertTrue(any(regions=={0,wing} for regions in edges.values()))
        self.assertTrue(all(math.isfinite(x) for p in v for x in p))

    def test_geometry_is_fourfold_symmetric_and_keeps_ridges(self):
        v,_,_=crown_mesh(subdivisions=4)
        original=Counter(tuple(round(x,7) for x in p) for p in v)
        rotated=Counter((round(-y,7),round(x,7),round(z,7)) for x,y,z in v)
        self.assertEqual(original,rotated)
        for p in ((0,7.4,43.2),(3.3,7.4,43.2),(0,.9,46.2),(0,12.1,39.3)):
            self.assertIn(p,original)

    def test_front_wing_has_no_triangulation_dependent_height_dimple(self):
        vertices,_,_=crown_mesh(subdivisions=8)
        checked=0
        for x,y,z in vertices[:len(vertices)//2]:
            if y<7.4-1e-8:continue
            t=(y-7.4)/4.7
            width=3.3+3*t
            if abs(x)>width+1e-8:continue
            expected=39.3+3.9*(1-t)**1.65+3.5*(abs(x)/width)**3*t**4
            self.assertAlmostEqual(z,expected,places=7)
            checked+=1
        self.assertGreater(checked,30)

    def test_invalid_arguments(self):
        for kw in ({'subdivisions':0},{'subdivisions':1.5},{'subdivisions':True},{'thickness':0},{'thickness':float('nan')}):
            with self.assertRaises(ValueError):crown_mesh(**kw)


if __name__=='__main__':unittest.main()
