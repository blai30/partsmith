from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0, 10), (30, 10), id="e-1")
        Line((30, 10), (30, 20), id="e-2")
        Line((30, 20), (0, 20), id="e-3")
        Line((0, 20), (0, 10), id="e-4")
    revolve(sketch_1, Axis.X, id="f-rev1")
