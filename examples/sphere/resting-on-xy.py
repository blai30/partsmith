from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    sphere(10, position=(0, 0, 10), id="f-sph1")
