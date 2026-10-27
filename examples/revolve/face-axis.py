from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_2:
    cylinder(5, 10, id="f-cyl1")
with body_1:
    with Sketch(Plane.XZ, id="f-sk1") as sketch_1:
        Line((10, 0), (20, 0), id="e-1")
        Line((20, 0), (20, 30), id="e-2")
        Line((20, 30), (10, 30), id="e-3")
        Line((10, 30), (10, 0), id="e-4")
    revolve(
        sketch_1,
        Ref("f-cyl1/side", "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5|adj=2,3", part=body_2, direction=(0, 0, 1)),
        90*deg,
        regions=[Region("e-1,e-2,e-3,e-4", "c=15,15|a=300")],
        id="f-rev1",
    )
