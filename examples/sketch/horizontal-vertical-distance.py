from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        circle_1 = Circle(5, center=(18, -13), id="e-1")
        HorizontalDistance(Sketch.ORIGIN, circle_1.center, 20*mm, id="c-1")
        VerticalDistance(circle_1.center, Sketch.ORIGIN, 15*mm, id="c-2")
    extrude(sketch_1, 10, id="f-ext1")
