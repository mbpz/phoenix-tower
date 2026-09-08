"""Estimated column-head masses fitted to a single-layer canopy underside.

These are not measured dougong or a structural load-path model. All sections,
heights and the fitting clearance are visual study choices.
"""
import math

SUPPORT_WIDTH = 1.04
SUPPORT_CLEARANCE = .02
MIN_SUPPORT_HEIGHT = .04


def perimeter_centres(centres, outline):
    """Select traced columns on the given perimeter, preserving input order."""
    selected=[]
    for x,y in centres:
        for (ax,ay),(bx,by) in zip(outline,outline[1:]+outline[:1]):
            if (abs((x-ax)*(by-ay)-(y-ay)*(bx-ax))<1e-8
                    and min(ax,bx)<=x<=max(ax,bx) and min(ay,by)<=y<=max(ay,by)):
                if (x,y) not in selected: selected.append((x,y))
                break
    return selected


def underside_probe(triangles, center, width):
    """Lowest underside point and source triangle over a clipped square.

    Input must be a triangulated, non-overlapping single-layer underside in XY.
    Coverage area rejects gaps for that contract, not arbitrary layered meshes.
    Minimum of clipped polygon vertices is exact for each planar triangle.
    """
    if width<=0 or not all(math.isfinite(v) for v in (*center,width)):
        raise ValueError('Finite center and positive footprint required')
    x,y=center
    bounds=[(0,x-width/2,1),(0,x+width/2,-1),
            (1,y-width/2,1),(1,y+width/2,-1)]
    area=0.0
    lowest=math.inf
    witness=None
    source_index=None
    for index,triangle in enumerate(triangles):
        if len(triangle)!=3 or any(len(p)!=3 or not all(math.isfinite(v) for v in p) for p in triangle):
            raise ValueError('Finite XYZ triangles required')
        a,b,c=triangle
        signed=(b[0]-a[0])*(c[1]-a[1])-(b[1]-a[1])*(c[0]-a[0])
        if signed>=0: continue  # Top surface and vertical rims are not the underside.
        polygon=list(triangle)
        for axis,edge,sign in bounds:
            clipped=[]
            for a,b in zip(polygon,polygon[1:]+polygon[:1]):
                da,db=(a[axis]-edge)*sign,(b[axis]-edge)*sign
                if da>=0: clipped.append(a)
                if (da>=0)!=(db>=0):
                    t=da/(da-db)
                    clipped.append(tuple(a[k]+t*(b[k]-a[k]) for k in range(3)))
            polygon=clipped
        if len(polygon)<3: continue
        projected=abs(sum(a[0]*b[1]-b[0]*a[1] for a,b in zip(polygon,polygon[1:]+polygon[:1])))/2
        if projected<=1e-12: continue
        area+=projected
        point=min(polygon,key=lambda p:(p[2],p[0],p[1]))
        if point[2]<lowest:
            lowest=point[2]
            witness=tuple(point)
            source_index=index
    if not math.isclose(area,width*width,rel_tol=1e-6,abs_tol=1e-8) or not math.isfinite(lowest):
        raise ValueError('Support footprint lacks single-layer underside coverage')
    return {'cap_m':lowest, 'minimum_point':witness,
            'triangle_index':source_index, 'projected_area':area}


def underside_cap(triangles, center, width):
    """Scalar compatibility API; shares the exact footprint clipping calculation."""
    return underside_probe(triangles,center,width)['cap_m']


def support_members(triangles, centres, bottom):
    """Four contiguous box descriptors per column; .02m below flat cap bound."""
    if not math.isfinite(bottom): raise ValueError('Finite post head required')
    triangles=list(triangles)
    members=[]
    for x,y in centres:
        top=underside_cap(triangles,(x,y),SUPPORT_WIDTH)-SUPPORT_CLEARANCE
        height=top-bottom
        if height<=MIN_SUPPORT_HEIGHT: raise ValueError('Insufficient space for support proxy')
        z=bottom
        for width,fraction in ((.48,.55),(.64,.15),(.84,.15),(SUPPORT_WIDTH,.15)):
            h=height*fraction
            members.append(((x,y,z+h/2),(width,width,h)))
            z+=h
    return members


def support_study(triangles, centres, bottom):
    """Retain insufficient-clearance cases in the report; never resize to hide them."""
    if not math.isfinite(bottom): raise ValueError('Finite post head required')
    triangles=list(triangles)
    result={'members':[], 'accepted':[], 'blocked':[]}
    for center in centres:
        cap=underside_cap(triangles,center,SUPPORT_WIDTH)
        if cap-SUPPORT_CLEARANCE-bottom<=MIN_SUPPORT_HEIGHT:
            result['blocked'].append({'center':center,'underside_cap_m':cap,
                                      'post_head_m':bottom,'reason':'Insufficient space for support proxy'})
        else:
            result['members'].extend(support_members(triangles,[center],bottom))
            result['accepted'].append(center)
    return result
