from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_2:
    box(10, 10, 10, position=(0, 5, 0), id="f-box1")
with body_1:
    with Sketch(Plane.XZ, id="f-sk1") as sketch_1:
        Line((10, 0), (20, 0), id="e-1")
        Line((20, 0), (20, 30), id="e-2")
        Line((20, 30), (10, 30), id="e-3")
        Line((10, 30), (10, 0), id="e-4")
    revolve(
        sketch_1,
        Ref("edge(f-box1/front|f-box1/left)", "line|n=0,0,1|c=-5,0,5|l=10", part=body_2),
        id="f-rev1",
    )
