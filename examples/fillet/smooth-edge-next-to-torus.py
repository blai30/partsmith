from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    cylinder(10, 20, id="f-cyl1")
    fillet(
        Ref("edge(f-cyl1/side|f-cyl1/top)", "circle|n=0,0,1|c=0,0,20|l=62.831853|r=10"),
        2,
        id="f-fil1",
    )
    fillet(
        Ref("edge(f-cyl1/side|f-fil1/fillet/edge(f-cyl1/side|f-cyl1/top))", "circle|n=0,0,1|c=0,0,18|l=62.831853|r=10"),
        1,
        id="f-fil2",
    )
