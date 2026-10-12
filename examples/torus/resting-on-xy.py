from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    torus(20, 5, position=(0, 0, 5), id="f-tor1")
