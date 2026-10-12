from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(30, 20, 10, id="f-box1")
    fillet(
        [Ref("edge(f-box1/front|f-box1/left)", "line|n=0,0,1|c=-15,-10,5|l=10"), Ref("edge(f-box1/front|f-box1/right)", "line|n=0,0,1|c=15,-10,5|l=10"), Ref("edge(f-box1/back|f-box1/left)", "line|n=0,0,1|c=-15,10,5|l=10"), Ref("edge(f-box1/back|f-box1/right)", "line|n=0,0,1|c=15,10,5|l=10")],
        3,
        id="f-fil1",
    )
    chamfer(
        Ref("edge(f-box1/front|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "line|n=0,0,1|c=-12,-10,5|l=10"),
        1,
        id="f-cha1",
    )
