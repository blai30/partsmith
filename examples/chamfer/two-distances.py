from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    feature(
        "chamfer",
        id="f-cha1",
        body="b-1",
        edges=[{"q": "edge(f-box1/front|f-box1/top)", "fp": "line|n=1,0,0|c=0,-10,20|l=20"}],
        distance={"kind": "two_distances", "expr": "1", "second_expr": "3"},
    )
