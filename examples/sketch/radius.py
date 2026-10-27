from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        line_1 = Line((0, 0), (40, 0), id="e-1")
        arc_1 = Arc(
            (20, -15),
            24,
            0.6435011087932844*rad,
            2.498091544796509*rad,
            id="e-2",
        )
        Coincident(line_1.start, Sketch.ORIGIN, id="c-1")
        Horizontal(line_1, id="c-2")
        Distance(line_1, 40*mm, id="c-3")
        Coincident(arc_1.end, Sketch.ORIGIN, id="c-4")
        Coincident(arc_1.start, line_1.end, id="c-5")
        Radius(arc_1, 25*mm, id="c-6")
    extrude(sketch_1, 10, id="f-ext1")
