from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 20, 10, id="f-box1")
    box(20, 20, 10, position=(10, 0, 10), id="f-box2")
    fillet(
        Ref("edge(f-box1/top|f-box2/left)", "line|n=0,1,0|c=0,0,10|l=20"),
        2,
        id="f-fil1",
    )
