from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(20, 20, 20, id="f-box1")
    box(20, 40, 20, position=(20, 0, 0), id="f-base1")
    chamfer(
        Ref("edge(f-box1/front|f-box1/top)", "line|n=1,0,0|c=0,-10,20|l=20|adj=2,2"),
        1,
        second_distance=3,
        id="f-cha1",
    )
