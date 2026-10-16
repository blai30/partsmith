from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Circle(10, id="e-5")
    extrude(sketch_1, 20, id="f-ext1")
