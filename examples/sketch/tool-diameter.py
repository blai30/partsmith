from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        circle_1 = Circle(12.5, id="e-1")
        Coincident(circle_1.center, Sketch.ORIGIN, id="c-1")
        Diameter(circle_1, 25*mm, id="c-2")
    extrude(sketch_1, 10, id="f-ext1")
