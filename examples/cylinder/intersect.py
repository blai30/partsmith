from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 10, id="f-box1")
    cylinder(5, 20, position=(0, 0, -5), op="intersect", id="f-cyl1")
