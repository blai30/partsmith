from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    sphere(8, position=(0, 0, 20), op="intersect", id="f-sph1")
