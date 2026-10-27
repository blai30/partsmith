from partsmith import *

doc = Document(units="mm", printer="generic-resin")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(80, 50, 30, id="f-box1")
    shell(None, 2, id="f-sh1")
