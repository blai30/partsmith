from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        point_1 = Point(12, 5, id="e-1")
        circle_1 = Circle(13, id="e-2")
        Fix(point_1, id="c-1")
        Coincident(circle_1.center, Sketch.ORIGIN, id="c-2")
        PointOn(point_1, circle_1, id="c-3")
    extrude(sketch_1, 10, id="f-ext1")
