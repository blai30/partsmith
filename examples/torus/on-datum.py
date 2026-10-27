from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    plane_1 = datum_plane(Plane.XZ, 10, id="f-dp1")
    torus(20, 5, plane=plane_1, id="f-tor1")
