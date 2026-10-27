from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((-30, -20), (30, -20), id="e-1")
        line_2 = Line((30, -20), (30, 20), id="e-2")
        line_3 = Line((30, 20), (-30, 20), id="e-3")
        line_4 = Line((-30, 20), (-30, -20), id="e-4")
        line_5 = Line((-30, -20), (30, 20), construction=True, id="e-5")
        point_1 = Point(0, 0, construction=True, id="e-6")
        Coincident(line_1.end, line_2.start, id="c-1")
        Coincident(line_2.end, line_3.start, id="c-2")
        Coincident(line_3.end, line_4.start, id="c-3")
        Coincident(line_4.end, line_1.start, id="c-4")
        Horizontal(line_1, id="c-5")
        Horizontal(line_3, id="c-6")
        Vertical(line_2, id="c-7")
        Vertical(line_4, id="c-8")
        Coincident(line_5.start, line_1.start, id="c-9")
        Coincident(line_5.end, line_2.end, id="c-10")
        Midpoint(point_1, line_5, id="c-11")
        Coincident(point_1, Sketch.ORIGIN, id="c-12")
        Distance(line_1, 60*mm, id="c-13")
        Distance(line_4, 40*mm, id="c-14")
    extrude(sketch_1, 10, id="f-ext1")
