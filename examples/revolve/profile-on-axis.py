from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XZ, id="f-sk1") as sketch_1:
        Line((0, 0), (10, 0), id="e-1")
        Line((10, 0), (10, 30), id="e-2")
        Line((10, 30), (0, 30), id="e-3")
        Line((0, 30), (0, 0), id="e-4")
    revolve(sketch_1, Axis.Z, id="f-rev1")
