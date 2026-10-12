from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")
body_3 = Part("Body 3", id="b-3")

with body_1:
    box(20, 20, 20, id="f-box1")
with body_2:
    box(20, 20, 20, position=(10, 0, 0), id="f-box2")
with body_3:
    box(10, 10, 10, position=(0, 0, 15), id="f-box3")
with body_1:
    boolean([body_2, body_3], id="f-bool1")
