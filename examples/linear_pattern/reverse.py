from partsmith import *

doc = Document(units="mm")

body_1 = Part("Body 1", id="b-1")
body_2 = Part("Body 2", id="b-2")

with body_1:
    box(10, 10, 10, id="f-box1")
    feature(
        "linear_pattern",
        id="f-lp1",
        body="b-1",
        direction={"axis": {"kind": "origin", "axis": "X"}, "count": "3", "spacing": "20", "reverse": True},
    )
