from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XZ, id="f-sk1") as sketch_1:
        Line((10, 0), (20, 0), id="e-1")
        Line((20, 30), (10, 30), id="e-3")
        Line((10, 30), (10, 0), id="e-4")
        Line((20, 0), (20, 15), id="e-2")
        Line((20, 15), (20, 30), id="e-6")
    revolve(
        sketch_1,
        Axis.Z,
        regions=[Region("e-1,e-2,e-3,e-4", "c=15,15|a=300", outline="e-1,e-2,e-3,e-4")],
        id="f-rev1",
    )
    shell(
        Ref("f-rev1/lateral/e-2", "cylinder|n=0,0,1|c=0,0,15|a=3769.911184|r=20|adj=2,3"),
        2,
        id="f-sh1",
    )
