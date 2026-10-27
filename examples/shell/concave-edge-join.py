from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 20, 10, id="f-box1")
    box(20, 20, 10, position=(10, 0, 10), id="f-box2")
    shell(Ref("f-box1/bottom", "plane|n=0,0,-1|c=0,0,0|a=800"), 2, id="f-sh1")
