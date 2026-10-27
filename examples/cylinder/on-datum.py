from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    plane_1 = datum_plane(Plane.XY, 5, flip=True, id="f-dp1")
    cylinder(5, 10, plane=plane_1, id="f-cyl1")
