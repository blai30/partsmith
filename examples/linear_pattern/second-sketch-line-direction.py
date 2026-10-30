from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 10, 10, id="f-box1")
    with Sketch(Plane.XY, id="f-sk1"):
        line_1 = Line((0, 0), (0, 1), construction=True, id="e-1")
    linear_pattern(Axis.X, 3, 20, second=(line_1, 2, 15), id="f-lp1")
