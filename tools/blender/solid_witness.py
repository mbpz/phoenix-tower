"""Read-only solid-angle diagnostic, independent of projected height clipping.

A winding number near +/-1 is an interior witness for a closed, consistently
oriented, non-self-intersecting shell. This checks edge closure/orientation but
NOT self-intersections, structural validity, or an entire contact volume.
Surface/near-surface points raise rather than being accepted as interior.
"""
import math
from collections import Counter


def _sub(a,b): return tuple(x-y for x,y in zip(a,b))
def _dot(a,b): return sum(x*y for x,y in zip(a,b))
def _cross(a,b):
    return (a[1]*b[2]-a[2]*b[1],a[2]*b[0]-a[0]*b[2],a[0]*b[1]-a[1]*b[0])


def winding_number(triangles, point, boundary_tolerance=1e-7):
    """Signed solid-angle sum / 4pi; all coordinates/tolerance use mesh units.

    Input is a triangle soup with exactly matching shared-edge coordinates.
    Boundary tolerance applies normal to the face and to its edge half-planes.
    Intended for the finite, metre-scale whitebox, not ill-conditioned CAD data.
    """
    point=tuple(point)
    if len(point)!=3 or not all(math.isfinite(x) for x in point):
        raise ValueError('Finite XYZ witness required')
    if not math.isfinite(boundary_tolerance) or boundary_tolerance<=0:
        raise ValueError('Positive finite boundary tolerance required')
    triangles=[tuple(tuple(p) for p in t) for t in triangles]
    edges=Counter(); terms=[]
    if not triangles: raise ValueError('Closed triangle shell required')
    for t in triangles:
        if len(t)!=3 or any(len(p)!=3 or not all(math.isfinite(x) for x in p) for p in t):
            raise ValueError('Finite XYZ triangles required')
        a,b,c=t
        normal=_cross(_sub(b,a),_sub(c,a)); length=math.sqrt(_dot(normal,normal))
        if length<=1e-14: raise ValueError('Degenerate triangle')
        normal=tuple(x/length for x in normal)
        for p,q in zip(t,t[1:]+t[:1]): edges[(p,q)]+=1
        distance=abs(_dot(_sub(point,a),normal))
        if distance<=boundary_tolerance and all(
                _dot(_cross(_sub(q,p),_sub(point,p)),normal)>=-boundary_tolerance*math.dist(p,q)
                for p,q in zip(t,t[1:]+t[:1])):
            raise ValueError('Witness lies on or too near shell boundary')
        vectors=[_sub(p,point) for p in t]
        norms=[math.sqrt(_dot(v,v)) for v in vectors]
        if min(norms)<=boundary_tolerance: raise ValueError('Witness too near shell vertex')
        u,v,w=[tuple(x/n for x in vec) for vec,n in zip(vectors,norms)]
        numerator=_dot(u,_cross(v,w))
        denominator=1+_dot(u,v)+_dot(v,w)+_dot(w,u)
        terms.append(2*math.atan2(numerator,denominator))
    if any(n!=1 or edges[(q,p)]!=1 for (p,q),n in edges.items()):
        raise ValueError('Shell must be closed with consistent edge orientation')
    return math.fsum(terms)/(4*math.pi)
