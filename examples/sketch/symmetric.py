from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((-19, 0), (21, 0), id="e-1")
        line_2 = Line((21, 0), (0, 30), id="e-2")
        line_3 = Line((0, 30), (-19, 0), id="e-3")
        Coincident(line_1.end, line_2.start, id="c-1")
        Coincident(line_2.end, line_3.start, id="c-2")
        Coincident(line_3.end, line_1.start, id="c-3")
        Symmetric(line_1.start, line_1.end, Sketch.Y_AXIS, id="c-4")
    extrude(sketch_1, 10, id="f-ext1")
