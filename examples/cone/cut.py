from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    cone(0, 5, 5, position=(0, 0, 15), op="cut", id="f-cone1")
