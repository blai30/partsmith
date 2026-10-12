from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    plane_1 = datum_plane(Plane.YZ, 20, id="f-dp1")
    box(10, 10, 10, position=(10, 0, 0), id="f-box1")
    mirror(plane_1, id="f-mir1")
