from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 20, 10, id="f-box1")
    box(20, 20, 10, position=(10, 0, 10), id="f-box2")
    fillet(
        Ref("edge(f-box1/front|f-box1/left)", "line|n=0,0,1|c=-20,-10,5|l=10"),
        1,
        id="f-fil1",
    )
    shell(Ref("f-box2/top", "plane|n=0,0,1|c=10,0,20|a=400"), 2, id="f-sh1")
