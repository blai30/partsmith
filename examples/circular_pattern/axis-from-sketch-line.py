from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    cylinder(2, 5, position=(20, 0, 0), id="f-cyl1")
    with Sketch(Plane.XZ, id="f-sk1"):
        line_1 = Line((0, 0), (0, 10), construction=True, id="e-1")
    circular_pattern(line_1, 3, 90*deg, id="f-cp1")
