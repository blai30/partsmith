from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XZ, id="f-sk1") as sketch_1:
        Line((10, 0), (20, 0), id="e-1")
        Line((20, 0), (20, 30), id="e-2")
        Line((20, 30), (10, 30), id="e-3")
        Line((10, 30), (10, 0), id="e-4")
        line_5 = Line((0, 10), (0, 0), construction=True, id="e-9")
    revolve(sketch_1, line_5, 90*deg, id="f-rev1")
