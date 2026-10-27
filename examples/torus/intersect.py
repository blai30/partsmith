from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(60, 60, 10, id="f-box1")
    torus(20, 5, op="intersect", id="f-tor1")
