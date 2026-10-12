from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(20, 20, 20, id="f-box1")
    split(Plane.XZ, into=body_2, id="f-split1")
