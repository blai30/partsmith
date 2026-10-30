from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    plane_1 = datum_plane(Plane.YZ, id="f-dp1")
    box(10, 10, 10, position=(10, 0, 0), id="f-box1")
    mirror(plane_1, id="f-mir1")
    with Sketch(
        Ref("f-box1/top@f-mir1:1", "plane|n=0,0,1|c=-10,0,10|a=100|adj=4,4", part=body_1),
        fallback=Placement((10, 0, 10), (-1, 0, 0), (0, 0, 1)),
        fallback_frame="body",
        id="f-sk2",
    ):
        point_1 = Point(2, 3, id="e-p1")
    hole(point_1, 1, id="f-hole1")
