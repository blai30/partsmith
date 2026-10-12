from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    cylinder(2, 5, position=(20, 0, 0), id="f-cyl1")
    circular_pattern(Axis.Z, 6, id="f-cp1")
