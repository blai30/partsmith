from partsmith import *

doc = Document(units="mm")

plane_1 = datum_plane(Plane.XY, 10, id="f-dp1")
datum_plane(plane_1, 5, id="f-dp2")
