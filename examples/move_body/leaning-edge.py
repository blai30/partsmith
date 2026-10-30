from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0, 0), (40, 0), id="e-1")
        Line((40, 0), (40, 20), id="e-2")
        Line((40, 20), (-1, 20), id="e-3")
        Line((-1, 20), (0, 0), id="e-4")
    extrude(
        sketch_1,
        10,
        regions=[Region("e-1,e-2,e-3,e-4", "c=20,10|a=800")],
        id="f-ext1",
    )
    move(
        rotate=(Ref("edge(f-ext1/lateral/e-4|f-ext1/start-cap)", "line|n=0,1,0|c=0,10,0|l=20|adj=2,2", part=body_1), 90*deg),
        id="f-mv1",
    )
