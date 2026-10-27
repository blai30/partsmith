from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 10, position=(0, 0, -10), id="f-box1")
    with Sketch(Plane.XY, id="f-sk1"):
        arc_1 = Arc((0, 0), 10, 0*rad, 1.5707963267948966*rad, id="e-6")
    hole([arc_1.start, arc_1.end, arc_1.center], 6, id="f-hole1")
