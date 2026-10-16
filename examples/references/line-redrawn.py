from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0, 0), (80, 0), id="e-1")
        Line((80, 50), (0, 50), id="e-3")
        Line((0, 50), (0, 0), id="e-4")
        Line((80, 0), (80, 50), id="e-9")
    extrude(
        sketch_1,
        30,
        regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000", outline="e-1,e-2,e-3,e-4")],
        id="f-ext1",
    )
    fillet(
        Ref("edge(f-ext1/end-cap|f-ext1/lateral/e-2)", "line|n=0,1,0|c=80,25,30|l=50|adj=2,2"),
        2,
        id="f-fil1",
    )
    with Sketch(
        Ref("f-ext1/lateral/e-2", "plane|n=1,0,0|c=80,25,14|a=1400|adj=4,4", part=body_1),
        fallback=Placement((80, 0, 0), (0, 1, 0), (1, 0, 0)),
        fallback_frame="body",
        id="f-sk2",
    ) as sketch_2:
        Circle(4, center=(25, 15), id="e-5")
    extrude(sketch_2, 5, reverse=True, op="cut", id="f-ext2")
