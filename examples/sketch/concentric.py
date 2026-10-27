from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        circle_1 = Circle(20, id="e-1")
        circle_2 = Circle(8, center=(0.6, -0.4), id="e-2")
        Coincident(circle_1.center, Sketch.ORIGIN, id="c-1")
        Concentric(circle_1, circle_2, id="c-2")
    extrude(
        sketch_1,
        10,
        regions=[Region("e-1;~e-2", "c=0,0|a=1055.575132", outline="e-1")],
        id="f-ext1",
    )
