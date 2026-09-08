"""Continuous crown study: traced plan regions, estimated shared curved profiles.

Not an as-built model. No Blender dependency: projected regions tile the crown,
then a single top surface is thickened vertically, without internal seam walls.
"""
import math


def _cross(a,b,c):
    return (b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])


def _triangulate(points):
    """Ear clipping for the small, fixed simple CCW control polygons below."""
    ids=list(range(len(points)));triangles=[]
    while len(ids)>3:
        for i,b in enumerate(ids):
            a,c=ids[i-1],ids[(i+1)%len(ids)]
            if _cross(points[a],points[b],points[c])<=1e-10:continue
            if any(all(_cross(points[u],points[v],points[p])>=-1e-10 for u,v in ((a,b),(b,c),(c,a))) for p in ids if p not in (a,b,c)):continue
            triangles.append((a,b,c));ids.pop(i);break
        else:raise ValueError('Invalid crown control polygon')
    triangles.append(tuple(ids))
    return triangles


def _base_height(x,y):
    radius=max(abs(x),abs(y))
    t=max(0,min(1,(radius-.9)/11.2))
    diagonal=min(abs(x),abs(y))/max(radius,.9)
    return 39.22+6.98*(1-t)**1.65+3.5*diagonal**6*t**4


def _wing_front_height(x,y):
    """Estimated profile: flat central run, corner lift outside 3.3 m shoulders.

    The shoulder reuses the provisional half-ridge width, NOT a measured eave
    dimension. At the widening hip x=3.3+3*t this retains the old 3.5*t**4 lift.
    """
    t=max(0,min(1,(y-7.4)/4.7))
    shoulder=max(0,(abs(x)-3.3)/3)
    return 39.3+3.9*(1-t)**1.65+3.5*shoulder**3*t


def crown_mesh(subdivisions=8, thickness=.18):
    """Return vertices, faces, and region IDs (0 main, 1–4 cardinal wings).

    Plan controls adapt plate 2-2-8; symmetrical 12.1 m half-width and mirrored
    folded seams are declared estimates. Section ridge levels stay 46.2/43.2 m.
    Per-triangle edge corrections share the SAME profile on each common edge.
    """
    if isinstance(subdivisions,bool) or not isinstance(subdivisions,int) or subdivisions<1 or not math.isfinite(thickness) or thickness<=0:
        raise ValueError('Positive integer subdivisions and finite thickness required')
    # One half of the north sector. Mirror/rotate, rather than independently
    # approximating four overlapping annular sheets.
    points={
        'I':(0,.9),'J':(.9,.9),'C':(12.1,12.1),'T':(6.3,12.1),
        'A':(5.9,8.2),'B':(4.7,7),'D':(3,5.9),'E':(0,5.9),
        'R':(3.3,7.4),'S':(0,7.4),'F':(0,12.1),
    }
    heights={key:_base_height(*p) for key,p in points.items()}
    heights.update(I=46.2,J=46.2,C=42.72,T=42.8,R=43.2,S=43.2,F=39.3)
    patches=[(0,['I','J','C','T','A','B','D','E']),
             (1,['R','D','B','A','T']),
             (1,['S','R','T','F']),
             (1,['E','D','R','S'])]
    profiles={
        ('C','T'):lambda t:39.22+3.5*abs(2*t-1)**3+.08*t,
        ('F','T'):lambda t:_wing_front_height(6.3*t,12.1),
        ('S','R'):lambda t:43.2,
        ('R','T'):lambda t:39.3+3.9*(1-t)**1.65+3.5*t**4,
        ('S','F'):lambda t:39.3+3.9*(1-t)**1.65,
    }
    def edge_delta(a,b,t):
        fn=profiles.get((a,b))
        if fn is None:
            fn=profiles.get((b,a))
            if fn is None:return 0
            t=1-t;a,b=b,a
        x,y=(points[a][i]*(1-t)+points[b][i]*t for i in (0,1))
        linear_delta=(heights[a]-_base_height(*points[a]))*(1-t)+(heights[b]-_base_height(*points[b]))*t
        return fn(t)-(_base_height(x,y)+linear_delta)

    vertices=[];faces=[];regions=[];cache={}
    def vertex(x,y,z):
        key=(round(x,9),round(y,9))
        if key in cache:
            idx=cache[key]
            if abs(vertices[idx][2]-z)>1e-7:raise ValueError('Mismatched crown seam height')
            return idx
        cache[key]=len(vertices);vertices.append((key[0],key[1],round(z,9)))
        return len(vertices)-1

    for quarter in range(4):
        for mirrored in (False,True):
            for region,polygon in patches:
                controls=[points[k] for k in polygon]
                for tri in _triangulate(controls):
                    keys=[polygon[i] for i in tri]
                    grid={}
                    for i in range(subdivisions+1):
                        for j in range(subdivisions+1-i):
                            weights=(1-(i+j)/subdivisions,i/subdivisions,j/subdivisions)
                            x,y=(sum(weights[k]*points[keys[k]][axis] for k in range(3)) for axis in (0,1))
                            z=_base_height(x,y)+sum(weights[k]*(heights[keys[k]]-_base_height(*points[keys[k]])) for k in range(3))
                            for a,b in ((0,1),(1,2),(2,0)):
                                amount=weights[a]+weights[b]
                                if amount>1e-10:
                                    z+=amount**4*edge_delta(keys[a],keys[b],weights[b]/amount)
                            # The front trapezoid has one analytic surface, independent
                            # of ear-clipping diagonals. Its four edges exactly match
                            # the common profiles above (ridge, hip, center and eave).
                            if polygon==['S','R','T','F']:
                                z=_wing_front_height(x,y)
                            if mirrored:x=-x
                            for _ in range(quarter):x,y=-y,x
                            grid[(i,j)]=vertex(x,y,z)
                    for i in range(subdivisions):
                        for j in range(subdivisions-i):
                            cells=[(grid[i,j],grid[i+1,j],grid[i,j+1])]
                            if i+j<subdivisions-1:cells.append((grid[i+1,j],grid[i+1,j+1],grid[i,j+1]))
                            for face in cells:
                                faces.append(list(reversed(face)) if mirrored else list(face))
                                regions.append(quarter+1 if region else 0)
    # One welded sheet, then one shell. Only the outer edge and finial aperture
    # get side walls; internal main/wing seams never receive duplicate walls.
    count=len(vertices)
    vertices += [(x,y,z-thickness) for x,y,z in vertices]
    top_faces=list(faces);top_regions=list(regions)
    boundary={}
    for face,region in zip(top_faces,top_regions):
        faces.append([i+count for i in reversed(face)]);regions.append(region)
        for a,b in zip(face,face[1:]+face[:1]):
            edge=tuple(sorted((a,b)))
            if edge in boundary:del boundary[edge]
            else:boundary[edge]=(a,b,region)
    for a,b,region in boundary.values():
        faces.append([b,a,a+count,b+count]);regions.append(region)
    return vertices,faces,regions
