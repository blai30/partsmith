from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    fillet(
        [Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,20|l=20"), Ref("edge(f-box1/back|f-box1/top)", "line|n=1,0,0|c=0,10,20|l=20"), Ref("edge(f-box1/left|f-box1/top)", "line|n=0,1,0|c=-10,0,20|l=20"), Ref("edge(f-box1/right|f-box1/top)", "line|n=0,1,0|c=10,0,20|l=20"), Ref("edge(f-box1/bottom|f-box1/front)", "line|n=1,0,0|c=0,-10,0|l=20"), Ref("edge(f-box1/back|f-box1/bottom)", "line|n=1,0,0|c=0,10,0|l=20"), Ref("edge(f-box1/bottom|f-box1/left)", "line|n=0,1,0|c=-10,0,0|l=20"), Ref("edge(f-box1/bottom|f-box1/right)", "line|n=0,1,0|c=10,0,0|l=20"), Ref("edge(f-box1/front|f-box1/left)", "line|n=0,0,1|c=-10,-10,10|l=20"), Ref("edge(f-box1/front|f-box1/right)", "line|n=0,0,1|c=10,-10,10|l=20"), Ref("edge(f-box1/back|f-box1/left)", "line|n=0,0,1|c=-10,10,10|l=20"), Ref("edge(f-box1/back|f-box1/right)", "line|n=0,0,1|c=10,10,10|l=20")],
        2,
        id="f-fil1",
    )
    move(translate=(50, 0, 0), id="f-mv1")
    shell(Ref("f-box1/top", "plane|n=0,0,1|c=50,0,20|a=256"), 1, id="f-sh1")
