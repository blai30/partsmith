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
    feature(
        "chamfer",
        id="f-cha1",
        body="b-1",
        edges=[{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,10|l=24"}, {"q": "edge(f-box1/top|f-fil1/fillet/edge(f-box1/front|f-box1/left))", "fp": "circle|n=0,0,1|c=-13.909859,-8.909859,10|l=4.712389|r=3"}],
        distance={"kind": "two_distances", "expr": "1", "second_expr": "2"},
    )
