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
    cylinder(2, 5, position=(20, 0, 0), id="f-cyl1")
    circular_pattern(
        Ref("edge(f-ext5/lateral/e-12|f-ext5/start-cap)", "line|n=0,0,1|c=-4,0,15|l=30|adj=2,2", part=body_2),
        2,
        90*deg,
        id="f-cp1",
    )
