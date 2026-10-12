from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 10, position=(0, 0, -10), id="f-box1")
    with Sketch(Plane.XY, id="f-sk1"):
        line_1 = Line((0, 0), (10, 0), id="e-1")
    hole(line_1.start, 6, id="f-hole1")
