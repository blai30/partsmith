from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")

with body_1:
    box(40, 40, 10, position=(0, 0, -10), id="f-box1")
    with Sketch(Plane.XY, id="f-sk1"):
        Point(0, 0, id="e-p1")
    feature(
        "hole",
        id="f-hole1",
        body="b-1",
        sketch="f-sk1",
        points=["e-p1", "e-p2"],
        diameter="6",
        extent={"kind": "through_all"},
    )
