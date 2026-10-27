from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(10, 10, 10, id="f-box1")
    box(10, 10, 10, position=(5, 5, 5), op="intersect", id="f-box2")
