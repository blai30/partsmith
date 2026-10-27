from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((0, 0), (80, 0), id="e-1")
        Line((80, 0), (80, 50), id="e-2")
        Line((80, 50), (0, 50), id="e-3")
        line_4 = Line((0, 50), (0, 0.3), id="e-4")
        Coincident(line_4.end, line_1.start, id="c-1")
    extrude(
        sketch_1,
        30,
        regions=[Region("e-1,e-2,e-3,e-4", "c=40,25|a=4000", outline="e-1,e-2,e-3,e-4")],
        id="f-ext1",
    )
