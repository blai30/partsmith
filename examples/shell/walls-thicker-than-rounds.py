from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 20, id="f-box1")
    fillet(
        [Ref("edge(f-box1/front|f-box1/left)", "line|n=0,0,1|c=-20,-20,10|l=20"), Ref("edge(f-box1/front|f-box1/right)", "line|n=0,0,1|c=20,-20,10|l=20"), Ref("edge(f-box1/back|f-box1/left)", "line|n=0,0,1|c=-20,20,10|l=20"), Ref("edge(f-box1/back|f-box1/right)", "line|n=0,0,1|c=20,20,10|l=20")],
        1,
        id="f-fil1",
    )
    shell(Ref("f-box1/top", "plane|n=0,0,1|c=0,0,20|a=1600"), 2, id="f-sh1")
