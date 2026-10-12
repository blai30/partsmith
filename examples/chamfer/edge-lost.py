from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    chamfer(
        [Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,20|l=20"), Ref("edge(f-box1/nothing|f-box1/top)", "line|n=0,0,1|c=500,500,500|l=1")],
        2,
        id="f-cha1",
    )
