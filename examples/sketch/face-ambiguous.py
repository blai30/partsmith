from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(10, 10, 10, id="f-box1")
with Sketch(
    Ref("f-box1/nothing", "plane|a=100", part=body_1),
    fallback=Placement((3, 4, 0), (1, 0, 0), (0, 0, 1)),
    id="f-sk1",
):
    Line((0, 0), (80, 0), id="e-1")
    Line((80, 0), (80, 50), id="e-2")
    Line((80, 50), (0, 50), id="e-3")
    Line((0, 50), (0, 0), id="e-4")
