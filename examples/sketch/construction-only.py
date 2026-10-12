from partsmith import *

doc = Document(units="mm")

with Sketch(Plane.XY, id="f-sk1"):
    Line((0, 0), (10, 0), construction=True, id="e-1")
