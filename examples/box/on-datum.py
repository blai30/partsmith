from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    plane_1 = datum_plane(Plane.XY, 5, id="f-dp1")
    box(10, 20, 30, plane=plane_1, id="f-box1")
