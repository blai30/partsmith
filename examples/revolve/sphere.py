from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XZ, id="f-sk1") as sketch_1:
        Arc((0, 0), 10, -1.5707963267948966*rad, 1.5707963267948966*rad, id="e-1")
        Line((0, 10), (0, -10), id="e-2")
    revolve(sketch_1, Axis.Z, id="f-rev1")
