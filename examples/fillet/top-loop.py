from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(30, 20, 10, id="f-box1")
    fillet(
        [Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,10|l=30"), Ref("edge(f-box1/back|f-box1/top)", "line|n=1,0,0|c=0,10,10|l=30"), Ref("edge(f-box1/left|f-box1/top)", "line|n=0,1,0|c=-15,0,10|l=20"), Ref("edge(f-box1/right|f-box1/top)", "line|n=0,1,0|c=15,0,10|l=20")],
        2,
        id="f-fil1",
    )
