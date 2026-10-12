from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 10, 10, position=(5, 0, 0), id="f-box1")
    mirror(Plane.YZ, id="f-mir1")
