from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(10, 20, 30, id="f-box1")
    move(translate=(5, 0, 0), rotate=(Axis.Z, 90*deg), id="f-mv1")
