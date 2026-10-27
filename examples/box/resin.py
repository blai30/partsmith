from partsmith import *

doc = Document(units="mm", printer="generic-resin")

wall = param("wall", "6 * printer.layer_height", id="p-wall")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(140, wall, 20, id="f-box1")
with body_2:
    box(20, 20, 10, position=(0, 30, 0), id="f-box2")
    chamfer(
        Ref("edge(f-box2/bottom|f-box2/front)", "line|n=1,0,0|c=0,20,0|l=20"),
        5,
        id="f-cha1",
    )
