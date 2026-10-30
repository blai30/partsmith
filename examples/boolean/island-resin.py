from partsmith import *

doc = Document(units="mm", printer="generic-resin")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")
body_3 = Part("Body 3", id="b-3")

with body_1:
    box(4, 10, 20, position=(2, 5, 0), id="f-box1")
with body_2:
    box(16, 10, 4, position=(12, 5, 16), id="f-box2")
with body_3:
    box(4, 10, 8, position=(18, 5, 8), id="f-box3")
with body_1:
    boolean([body_2, body_3], op="join", id="f-bool1")
