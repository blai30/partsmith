from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 10, position=(0, 0, -10), id="f-box1")
    plane_1 = datum_plane(Plane.XY, -5, id="f-dp1")
    with Sketch(plane_1, id="f-sk1"):
        point_1 = Point(0, 0, id="e-p1")
        Point(10, 10, id="e-p2")
        Point(100, 0, id="e-p3")
    hole(point_1, 6, id="f-hole1")
