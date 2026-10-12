from partsmith import *

doc = Document(units="mm")

plane_1 = datum_plane(Plane.XY, 10, id="f-dp1")
with Sketch(plane_1, id="f-sk1"):
    Line((0, 0), (80, 0), id="e-1")
    Line((80, 0), (80, 50), id="e-2")
    Line((80, 50), (0, 50), id="e-3")
    Line((0, 50), (0, 0), id="e-4")
