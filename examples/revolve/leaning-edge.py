from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_2:
    with Sketch(Plane.XZ, id="f-sk5") as sketch_1:
        Line((-6, 0), (-4, 0), id="e-11")
        Line((-4, 0), (-5, 30), id="e-12")
        Line((-5, 30), (-6, 30), id="e-13")
        Line((-6, 30), (-6, 0), id="e-14")
    extrude(
        sketch_1,
        10,
        regions=[Region("e-11,e-12,e-13,e-14", "c=-5,15|a=60")],
        id="f-ext5",
    )
with body_1:
    with Sketch(Plane.XZ, id="f-sk1") as sketch_2:
        Line((10, 0), (20, 0), id="e-1")
        Line((20, 0), (20, 30), id="e-2")
        Line((20, 30), (10, 30), id="e-3")
        Line((10, 30), (10, 0), id="e-4")
    revolve(
        sketch_2,
        Ref("edge(f-ext5/lateral/e-12|f-ext5/start-cap)", "line|n=0,0,1|c=-4,0,15|l=30|adj=2,2", part=body_2, direction=(0, 0, 1)),
        90*deg,
        regions=[Region("e-1,e-2,e-3,e-4", "c=15,15|a=300")],
        id="f-rev1",
    )
