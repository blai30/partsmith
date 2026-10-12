from partsmith import *

doc = Document(units="in")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0*mm, 0*mm), (80*mm, 0*mm), id="e-1")
        Line((80*mm, 0*mm), (80*mm, 50*mm), id="e-2")
        Line((80*mm, 50*mm), (0*mm, 50*mm), id="e-3")
        Line((0*mm, 50*mm), (0*mm, 0*mm), id="e-4")
    extrude(sketch_1, 1, id="f-ext1")
