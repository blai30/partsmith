from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Circle(10, id="e-5")
        Line((-10, 0), (10, 0), id="e-6")
    extrude(
        sketch_1,
        20,
        regions=[Region("e-5,e-6", "c=0,4.244132|a=157.079633"), Region("e-5,~e-6", "c=0,-4.244132|a=157.079633")],
        id="f-ext1",
    )
