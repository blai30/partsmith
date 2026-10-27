from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(400, 300, 10, id="f-box1")
