from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 10, position=(0, 0, -10), id="f-box1")
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Circle(5, id="e-5")
    extrude(sketch_1, 5, regions=[Region("e-5", "c=0,0|a=78.539816")], id="f-ext1")
