from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    cylinder(20, 40, id="f-cyl1")
    shell(Ref("f-cyl1/top", "plane|n=0,0,1|c=0,0,40|a=1256.637061"), 2, id="f-sh1")
