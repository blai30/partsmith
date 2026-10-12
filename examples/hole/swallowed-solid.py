from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 10, position=(0, 0, -10), id="f-box1")
    box(2, 2, 2, position=(30, 0, -6), id="f-box2")
    with Sketch(Plane.XY, id="f-sk1"):
        point_1 = Point(0, 0, id="e-p1")
        point_2 = Point(30, 0, id="e-p4")
        point_3 = Point(25, 15, id="e-p5")
    hole([point_1, point_2, point_3], 6, id="f-hole1")
