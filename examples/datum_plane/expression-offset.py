from partsmith import *

doc = Document(units="mm")

wall = param("wall", 2*mm, id="p-wall")

datum_plane(Plane.XY, "2 * wall", id="f-dp1")
