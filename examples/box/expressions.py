from partsmith import *

doc = Document(units="mm")

w = param("w", 8*mm, id="p-w")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(w, "2 * w", "w / 2", id="f-box1")
