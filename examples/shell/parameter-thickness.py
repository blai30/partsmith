from partsmith import *

doc = Document(units="mm")

wall = param("wall", 2*mm, id="p-wall")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(80, 50, 30, id="f-box1")
    shell(Ref("f-box1/top", "plane|n=0,0,1|c=0,0,30|a=4000"), wall, id="f-sh1")
