from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    cylinder(5, 10, id="f-cyl1")
with Sketch(
    Ref("f-cyl1/side", "cylinder|n=0,0,1|c=0,0,5|a=314.159265|r=5", part=body_1),
    fallback=Placement((0, 0, 0), (1, 0, 0), (0, 0, 1)),
    id="f-sk1",
):
    Line((0, 0), (80, 0), id="e-1")
    Line((80, 0), (80, 50), id="e-2")
    Line((80, 50), (0, 50), id="e-3")
    Line((0, 50), (0, 0), id="e-4")
