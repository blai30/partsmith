from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((0, 0), (40, 0), id="e-1")
        arc_1 = Arc(
            (40, 10),
            10,
            4.71238898038469*rad,
            1.5707963267948966*rad,
            id="e-2",
        )
        line_2 = Line((40, 20), (0, 20), id="e-3")
        line_3 = Line((0, 20), (0, 0), id="e-4")
        Coincident(line_1.start, Sketch.ORIGIN, id="c-1")
        Horizontal(line_1, id="c-2")
        Distance(line_1, 40*mm, id="c-3")
        Coincident(arc_1.start, line_1.end, id="c-4")
        Tangent(line_1, arc_1, id="c-5")
        Coincident(line_2.start, arc_1.end, id="c-6")
        PointOn(line_2.end, Sketch.Y_AXIS, id="c-7")
        Tangent(line_2, arc_1, id="c-8")
        Coincident(line_2.end, line_3.start, id="c-9")
        Coincident(line_3.end, Sketch.ORIGIN, id="c-10")
    extrude(sketch_1, 10, id="f-ext1")
