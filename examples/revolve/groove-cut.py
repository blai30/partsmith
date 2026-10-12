from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    cylinder(30, 30, id="f-cyl1")
    with Sketch(Plane.XZ, id="f-sk1") as sketch_1:
        Line((10, 20), (20, 20), id="e-1")
        Line((20, 20), (20, 30), id="e-2")
        Line((20, 30), (10, 30), id="e-3")
        Line((10, 30), (10, 20), id="e-4")
    revolve(sketch_1, Axis.Z, op="cut", id="f-rev1")
