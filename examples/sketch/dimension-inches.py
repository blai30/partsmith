from partsmith import *

doc = Document(units="in")

with Sketch(Plane.XY, id="f-sk1"):
    line_1 = Line((0*mm, 0*mm), (80*mm, 0*mm), id="e-1")
    Fix(line_1.start, id="c-1")
    Horizontal(line_1, id="c-2")
    Distance(line_1, 2, id="c-3")
