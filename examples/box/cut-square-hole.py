from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 10, id="f-box1")
    box(10, 10, 20, position=(0, 0, -5), op="cut", id="f-box2")
