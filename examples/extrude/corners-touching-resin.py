from partsmith import *

doc = Document(units="mm", printer="generic-resin")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((-10, -10), (0, -10), id="e-1")
        Line((0, -10), (0, 0), id="e-2")
        Line((0, 0), (-10, 0), id="e-3")
        Line((-10, 0), (-10, -10), id="e-4")
        Line((0, 0), (10, 0), id="e-5")
        Line((10, 0), (10, 10), id="e-6")
        Line((10, 10), (0, 10), id="e-7")
        Line((0, 10), (0, 0), id="e-8")
    extrude(sketch_1, 10, id="f-ext1")
