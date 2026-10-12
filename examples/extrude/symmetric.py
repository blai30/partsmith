from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0, 0), (80, 0), id="e-1")
        Line((80, 0), (80, 50), id="e-2")
        Line((80, 50), (0, 50), id="e-3")
        Line((0, 50), (0, 0), id="e-4")
    extrude(sketch_1, 30*mm, symmetric=True, id="f-ext1")
