from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0, 0), (80, 0), id="e-1")
        Line((80, 0), (80, 50), id="e-2")
        Line((0, 50), (0, 0), id="e-4")
        Line((80, 50), (50, 50), id="e-9")
        Line((50, 50), (50, 40), id="e-10")
        Line((50, 40), (30, 40), id="e-11")
        Line((30, 40), (30, 50), id="e-12")
        Line((30, 50), (0, 50), id="e-13")
    extrude(
        sketch_1,
        30,
        regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000", outline="e-1,e-2,e-3,e-4")],
        id="f-ext1",
    )
    fillet(
        Ref("edge(f-ext1/end-cap|f-ext1/lateral/e-3)", "line|n=1,0,0|c=40,50,30|l=80|adj=2,2"),
        1,
        id="f-fil1",
    )
