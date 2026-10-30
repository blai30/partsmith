from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0, 0), (80, 0), id="e-1")
        Line((80, 50), (0, 50), id="e-3")
        Line((0, 50), (0, 0), id="e-4")
        Line((80, 0), (80, 25), id="e-2")
        Line((80, 25), (80, 50), id="e-6")
    extrude(
        sketch_1,
        30,
        regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000", outline="e-1,e-2,e-3,e-4")],
        id="f-ext1",
    )
    shell(
        Ref("f-ext1/lateral/e-2", "plane|n=1,0,0|c=80,25,15|a=1500|adj=4,4"),
        2,
        id="f-sh1",
    )
