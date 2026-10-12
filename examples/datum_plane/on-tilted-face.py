from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    chamfer(
        Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,20|l=20"),
        2,
        id="f-cha1",
    )
datum_plane(
    Ref("f-cha1/chamfer/edge(f-box1/front|f-box1/top)", "plane|n=0,-0.707107,0.707107|c=0,-9,19|a=56.568542", part=body_1),
    id="f-dp1",
)
