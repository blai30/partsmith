from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    cone(5, 5.0000005, 10, id="f-cone1")
