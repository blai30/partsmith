from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 20, 20, id="f-box1")
    torus(10, 2, plane=Plane.XZ, position=(0, 20, 0), op="cut", id="f-tor1")
