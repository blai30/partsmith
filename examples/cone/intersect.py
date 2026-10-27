from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(30, 30, 15, position=(0, 0, -5), id="f-box1")
    cone(10, 0, 20, op="intersect", id="f-cone1")
