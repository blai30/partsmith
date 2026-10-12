from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    with Sketch(Plane.XY, id="f-sk1") as sketch_1:
        Line((0, 0), (80, 0), id="e-1")
        Line((80, 0), (80, 50), id="e-2")
        Line((80, 50), (0, 50), id="e-3")
        Line((0, 50), (0, 0), id="e-4")
        Circle(10, center=(40, 25), id="e-5")
    extrude(
        sketch_1,
        10,
        regions=[Region("e-1,e-2,e-3,e-4;~e-5", "c=40,25|a=3685.840735"), Region("e-5", "c=40,25|a=314.159265")],
        id="f-ext1",
    )
