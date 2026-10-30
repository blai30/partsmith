from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((1, 2), (38, -1), id="e-1")
        line_2 = Line((38, -1), (41, 23), id="e-2")
        line_3 = Line((41, 23), (-2, 26), id="e-3")
        line_4 = Line((-2, 26), (1, 2), id="e-4")
        Coincident(line_1.end, line_2.start, id="c-1")
        Coincident(line_2.end, line_3.start, id="c-2")
        Coincident(line_3.end, line_4.start, id="c-3")
        Coincident(line_4.end, line_1.start, id="c-4")
        Coincident(line_1.start, Sketch.ORIGIN, id="c-5")
        Horizontal(line_1.start, line_1.end, id="c-6")
        Vertical(line_2.start, line_2.end, id="c-7")
        Horizontal(line_3.start, line_3.end, id="c-8")
        Vertical(line_4.start, line_4.end, id="c-9")
        HorizontalDistance(line_1, 40*mm, id="c-10")
        VerticalDistance(line_2, 25*mm, id="c-11")
    extrude(sketch_1, 10, id="f-ext1")
