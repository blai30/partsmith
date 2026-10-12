from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(30, 10, 10, id="f-box1")
    box(10, 10, 8, position=(0, 0, 2), op="cut", id="f-box2")
    plane_1 = datum_plane(Plane.XY, 5, id="f-dp1")
    split(plane_1, into=body_2, id="f-split1")
