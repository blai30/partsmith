from partsmith import *

doc = Document(units="mm")

width = param("width", 100*mm, id="p-w")

with Sketch(Plane.XY, id="f-sk1"):
    line_1 = Line((0, 0), (80, 0), id="e-1")
    Fix(line_1.start, id="c-1")
    Horizontal(line_1, id="c-2")
    Distance(line_1, width, id="c-3")
    Distance(line_1, "wdth", driving=False, id="c-4")
