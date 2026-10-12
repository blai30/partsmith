from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    box(20, 20, 20, position=(50, 0, 0), id="f-box2")
    shell(
        [Ref("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=400"), Ref("f-box2/top", "plane|n=0,0,1|c=50,0,20|a=400")],
        2,
        id="f-sh1",
    )
