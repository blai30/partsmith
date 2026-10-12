from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    cylinder(10, 60, plane=Plane.YZ, position=(0, 0, -30), id="f-cyl1")
    cylinder(6, 25, id="f-cyl2")
    fillet(
        Ref("edge(f-cyl1/side|f-cyl2/side)", "bspline_curve|c=0,0,9.026701|l=38.725445"),
        2,
        id="f-fil1",
    )
    fillet(
        Ref("edge(f-cyl2/side|f-fil1/fillet/edge(f-cyl1/side|f-cyl2/side))", "bspline_curve|c=0,0,10.524948|l=40.038448"),
        1,
        id="f-fil2",
    )
