from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(10, 20, 30, id="f-box1")
    with Sketch(Plane.XY, id="f-sk1"):
        line_1 = Line((5, -10), (5, 10), construction=True, id="e-1")
    move(rotate=(line_1, 90*deg), id="f-mv1")
