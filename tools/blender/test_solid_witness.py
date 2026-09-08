"""Independent interior-point checks, not a roof-height clipping regression."""
import math
import unittest
from solid_witness import winding_number


def tetrahedron():
    a,b,c,d=(0.,0.,0.),(2.,0.,0.),(0.,2.,0.),(0.,0.,2.)
    return [(a,c,b),(a,b,d),(a,d,c),(b,c,d)]


class SolidWitnessTests(unittest.TestCase):
    def test_inside_outside_and_reversed_orientation(self):
        triangles=tetrahedron()
        self.assertAlmostEqual(winding_number(triangles,(.2,.2,.2)),1.)
        self.assertAlmostEqual(winding_number(triangles,(2.,2.,2.)),0.)
        self.assertAlmostEqual(winding_number([t[::-1] for t in triangles],(.2,.2,.2)),-1.)

    def test_translated_geometry_and_interior_controls(self):
        offset=(1000.,-2000.,3000.)
        triangles=[tuple(tuple(p[k]+offset[k] for k in range(3)) for p in t) for t in tetrahedron()]
        point=tuple(offset[k]+.3 for k in range(3))
        self.assertAlmostEqual(winding_number(triangles,point),1.)
        self.assertAlmostEqual(winding_number(tetrahedron(),(-.01,.2,.2)),0.)
        self.assertAlmostEqual(winding_number(tetrahedron(),(.01,.2,.2)),1.)

    def test_surface_vertex_edge_and_face_are_not_interior_witnesses(self):
        for p in [(0.,0.,0.),(1.,0.,0.),(.2,.2,0.),(.2,.2,1.6),(.2,.2,1e-8)]:
            with self.subTest(p=p),self.assertRaises(ValueError):
                winding_number(tetrahedron(),p)

    def test_open_duplicate_and_bad_orientation_are_rejected(self):
        ts=tetrahedron()
        for triangles in [[],ts[:-1],ts+ts,[ts[0][::-1]]+ts[1:],ts+[(ts[0][0],)*3]]:
            with self.assertRaises(ValueError): winding_number(triangles,(.2,.2,.2))

    def test_nonfinite_and_invalid_tolerance_are_rejected(self):
        for p in [(math.nan,0,0),(0,math.inf,0),(0,0)]:
            with self.assertRaises(ValueError): winding_number(tetrahedron(),p)
        for tol in [0,-1,math.nan,math.inf]:
            with self.assertRaises(ValueError): winding_number(tetrahedron(),(.2,.2,.2),tol)


if __name__=='__main__': unittest.main()
