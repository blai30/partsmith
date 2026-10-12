from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    cylinder(2, 5, position=(20, 0, 0), id="f-cyl1")
with body_2:
    cylinder(5, 5, id="f-cyl2")
with body_1:
    circular_pattern(
        Ref("f-cyl2/side", "cylinder|n=0,0,1|c=0,0,2.5|a=157.079633|r=5", part=body_2),
        6,
        id="f-cp1",
    )
