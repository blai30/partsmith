from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((0, 0), (51.96152422706632, 30), id="e-1")
        line_2 = Line((51.96152422706632, 30), (0, 30), id="e-2")
        line_3 = Line((0, 30), (0, 0), id="e-3")
        Coincident(line_1.start, Sketch.ORIGIN, id="c-1")
        Distance(line_1, 60*mm, id="c-2")
        Angle(Sketch.X_AXIS, line_1, 30*deg, id="c-3")
        Coincident(line_1.end, line_2.start, id="c-4")
        PointOn(line_2.end, Sketch.Y_AXIS, id="c-5")
        Horizontal(line_2, id="c-6")
        Coincident(line_2.end, line_3.start, id="c-7")
        Coincident(line_3.end, Sketch.ORIGIN, id="c-8")
    extrude(sketch_1, 10, id="f-ext1")
