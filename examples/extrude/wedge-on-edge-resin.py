from partsmith import *

doc = Document(units="mm", printer="generic-resin")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XZ, id="f-sk1") as sketch_1:
        Line((0, 0), (5, 20), id="e-1")
        Line((5, 20), (-5, 20), id="e-2")
        Line((-5, 20), (0, 0), id="e-3")
    extrude(sketch_1, 10, id="f-ext1")
