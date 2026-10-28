from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XZ, id="f-sk1") as sketch_1:
        Line((10, 0), (20, 0), id="e-1")
        Line((20, 0), (20, 30), id="e-2")
        Line((20, 30), (10, 30), id="e-3")
        Line((10, 30), (10, 0), id="e-4")
        Line((15, 0), (15, 30), id="e-5")
    revolve(
        sketch_1,
        Axis.Z,
        regions=[Region("e-1,e-3,e-4,e-5", "c=12.5,15|a=150"), Region("e-1,e-2,e-3,~e-5", "c=17.5,15|a=150")],
        id="f-rev1",
    )
