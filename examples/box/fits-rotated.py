from partsmith import *

doc = Document(units="mm", printer="generic-resin")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(70, 100, 10, id="f-box1")
