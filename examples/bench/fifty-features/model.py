from partsmith import *

doc = Document(units="mm")

base = param(
    "base",
    240*mm,
    comment="Plate length; drives the first sketch.",
    id="p-base",
)
lid_width = param(
    "lid_width",
    24*mm,
    comment="Lid width; drives the last five features.",
    id="p-lid",
)

base_2 = Part("Base", id="b-1")
fins = Part("Fins", id="b-2")
bins = Part("Bins", id="b-3")
lids = Part("Lids", id="b-4")

with base_2:
    plane_1 = datum_plane(Plane.XY, 8*mm, id="f-dp1")
    plane_2 = datum_plane(Plane.YZ, 30*mm, id="f-dp2")
    with Sketch(plane_1, id="f-sk1") as sketch_1:
        line_1 = Line((-120, -80), (120, -80), id="e-101")
        line_2 = Line((120, -80), (120, 80), id="e-102")
        line_3 = Line((120, 80), (-120, 80), id="e-103")
        line_4 = Line((-120, 80), (-120, -80), id="e-104")
        circle_1 = Circle(3.25, center=(-108, -68), construction=True, id="e-105")
        circle_2 = Circle(3.25, center=(108, -68), construction=True, id="e-106")
        circle_3 = Circle(3.25, center=(108, 68), construction=True, id="e-107")
        circle_4 = Circle(3.25, center=(-108, 68), construction=True, id="e-108")
        Coincident(line_1.end, line_2.start, id="c-101")
        Coincident(line_2.end, line_3.start, id="c-102")
        Coincident(line_3.end, line_4.start, id="c-103")
        Coincident(line_4.end, line_1.start, id="c-104")
        Symmetric(line_1.start, line_1.end, Sketch.Y_AXIS, id="c-105")
        Symmetric(line_2.start, line_2.end, Sketch.X_AXIS, id="c-106")
        Horizontal(line_3, id="c-107")
        Vertical(line_4, id="c-108")
        Distance(line_1, base, id="c-109")
        Distance(line_2, 160*mm, id="c-110")
        HorizontalDistance(line_1.start, circle_1.center, 12*mm, id="c-111")
        VerticalDistance(line_1.start, circle_1.center, 12*mm, id="c-112")
        HorizontalDistance(circle_2.center, line_2.start, 12*mm, id="c-113")
        VerticalDistance(line_2.start, circle_2.center, 12*mm, id="c-114")
        HorizontalDistance(circle_3.center, line_3.start, 12*mm, id="c-115")
        VerticalDistance(circle_3.center, line_3.start, 12*mm, id="c-116")
        HorizontalDistance(line_4.start, circle_4.center, 12*mm, id="c-117")
        VerticalDistance(circle_4.center, line_4.start, 12*mm, id="c-118")
    extrude(
        sketch_1,
        8*mm,
        regions=[Region("e-101,e-102,e-103,e-104", "c=0,0|a=38400")],
        reverse=True,
        id="f-ext1",
    )
    fillet(
        [Ref("edge(f-ext1/lateral/e-101|f-ext1/lateral/e-102)", "line|n=0,0,1|c=120,-80,4|l=8"), Ref("edge(f-ext1/lateral/e-102|f-ext1/lateral/e-103)", "line|n=0,0,1|c=120,80,4|l=8"), Ref("edge(f-ext1/lateral/e-103|f-ext1/lateral/e-104)", "line|n=0,0,1|c=-120,80,4|l=8"), Ref("edge(f-ext1/lateral/e-101|f-ext1/lateral/e-104)", "line|n=0,0,1|c=-120,-80,4|l=8")],
        6*mm,
        id="f-fil1",
    )
    hole(
        [circle_1.center, circle_2.center, circle_3.center, circle_4.center],
        3.4*mm,
        counterbore=(6.5*mm, 3.5*mm),
        id="f-hole1",
    )
    chamfer(
        [Ref("edge(f-ext1/lateral/e-101|f-ext1/start-cap)", "line|n=1,0,0|c=0,-80,8|l=228"), Ref("edge(f-ext1/lateral/e-102|f-ext1/start-cap)", "line|n=0,1,0|c=120,0,8|l=148"), Ref("edge(f-ext1/lateral/e-103|f-ext1/start-cap)", "line|n=1,0,0|c=0,80,8|l=228"), Ref("edge(f-ext1/lateral/e-104|f-ext1/start-cap)", "line|n=0,1,0|c=-120,0,8|l=148")],
        1*mm,
        id="f-cha1",
    )
    with Sketch(plane_1, id="f-sk2") as sketch_2:
        line_1_2 = Line((-20, -20), (20, -20), id="e-201")
        line_2_2 = Line((20, -20), (20, 20), id="e-202")
        line_3_2 = Line((20, 20), (-20, 20), id="e-203")
        line_4_2 = Line((-20, 20), (-20, -20), id="e-204")
        circle_1_2 = Circle(2, center=(-12, -12), construction=True, id="e-205")
        circle_2_2 = Circle(2, center=(12, -12), construction=True, id="e-206")
        circle_3_2 = Circle(2, center=(12, 12), construction=True, id="e-207")
        circle_4_2 = Circle(2, center=(-12, 12), construction=True, id="e-208")
        Coincident(line_1_2.end, line_2_2.start, id="c-201")
        Coincident(line_2_2.end, line_3_2.start, id="c-202")
        Coincident(line_3_2.end, line_4_2.start, id="c-203")
        Coincident(line_4_2.end, line_1_2.start, id="c-204")
        Symmetric(line_1_2.start, line_1_2.end, Sketch.Y_AXIS, id="c-205")
        Symmetric(line_2_2.start, line_2_2.end, Sketch.X_AXIS, id="c-206")
        Horizontal(line_3_2, id="c-207")
        Vertical(line_4_2, id="c-208")
        Distance(line_1_2, 40*mm, id="c-209")
        Distance(line_2_2, 40*mm, id="c-210")
    extrude(
        sketch_2,
        20*mm,
        regions=[Region("e-201,e-202,e-203,e-204", "c=0,0|a=1600")],
        id="f-ext2",
    )
    fillet(
        [Ref("edge(f-ext2/lateral/e-201|f-ext2/lateral/e-202)", "line|n=0,0,1|c=20,-20,18|l=20"), Ref("edge(f-ext2/lateral/e-202|f-ext2/lateral/e-203)", "line|n=0,0,1|c=20,20,18|l=20"), Ref("edge(f-ext2/lateral/e-203|f-ext2/lateral/e-204)", "line|n=0,0,1|c=-20,20,18|l=20"), Ref("edge(f-ext2/lateral/e-201|f-ext2/lateral/e-204)", "line|n=0,0,1|c=-20,-20,18|l=20")],
        4*mm,
        id="f-fil2",
    )
    hole(
        [circle_1_2.center, circle_2_2.center, circle_3_2.center, circle_4_2.center],
        4*mm,
        reverse=True,
        id="f-hole2",
    )
    chamfer(
        [Ref("edge(f-ext2/end-cap|f-ext2/lateral/e-201)", "line|n=1,0,0|c=0,-20,28|l=32"), Ref("edge(f-ext2/end-cap|f-ext2/lateral/e-202)", "line|n=0,1,0|c=20,0,28|l=32"), Ref("edge(f-ext2/end-cap|f-ext2/lateral/e-203)", "line|n=1,0,0|c=0,20,28|l=32"), Ref("edge(f-ext2/end-cap|f-ext2/lateral/e-204)", "line|n=0,1,0|c=-20,0,28|l=32")],
        1*mm,
        id="f-cha2",
    )
with fins:
    with Sketch(plane_2, id="f-sk3") as sketch_3:
        line_1_3 = Line((-32, 8), (-16, 8), id="e-301")
        line_2_3 = Line((-16, 8), (-16, 22), id="e-302")
        line_3_3 = Line((-16, 22), (-32, 22), id="e-303")
        line_4_3 = Line((-32, 22), (-32, 8), id="e-304")
        circle_1_3 = Circle(3, center=(-24, 15), id="e-305")
        Coincident(line_1_3.end, line_2_3.start, id="c-301")
        Coincident(line_2_3.end, line_3_3.start, id="c-302")
        Coincident(line_3_3.end, line_4_3.start, id="c-303")
        Coincident(line_4_3.end, line_1_3.start, id="c-304")
        Horizontal(line_1_3, id="c-305")
        Horizontal(line_3_3, id="c-306")
        Vertical(line_2_3, id="c-307")
        Vertical(line_4_3, id="c-308")
        Distance(line_1_3, 16*mm, id="c-309")
        Distance(line_2_3, 14*mm, id="c-310")
        Radius(circle_1_3, 3*mm, id="c-311")
    extrude(
        sketch_3,
        4*mm,
        regions=[Region("e-301,e-302,e-303,e-304;~e-305", "c=-24,15|a=195.725666")],
        id="f-ext3",
    )
    fillet(
        [Ref("edge(f-ext3/lateral/e-303|f-ext3/start-cap)", "line|n=0,1,0|c=30,-24,22|l=16"), Ref("edge(f-ext3/end-cap|f-ext3/lateral/e-303)", "line|n=0,1,0|c=34,-24,22|l=16"), Ref("edge(f-ext3/lateral/e-302|f-ext3/lateral/e-303)", "line|n=1,0,0|c=32,-16,22|l=4"), Ref("edge(f-ext3/lateral/e-303|f-ext3/lateral/e-304)", "line|n=1,0,0|c=32,-32,22|l=4")],
        1.5*mm,
        id="f-fil3",
    )
    linear_pattern(Axis.X, 3, 12*mm, id="f-lp1")
    linear_pattern(Axis.Y, 3, 24*mm, id="f-lp2")
    mirror(Plane.YZ, id="f-mir1")
with bins:
    with Sketch(plane_1, id="f-sk4") as sketch_4:
        line_1_4 = Line((-38, -66), (-18, -66), id="e-401")
        line_2_4 = Line((-18, -66), (-18, -46), id="e-402")
        line_3_4 = Line((-18, -46), (-38, -46), id="e-403")
        line_4_4 = Line((-38, -46), (-38, -66), id="e-404")
        Circle(4, center=(-28, -56), construction=True, id="e-405")
        Coincident(line_1_4.end, line_2_4.start, id="c-401")
        Coincident(line_2_4.end, line_3_4.start, id="c-402")
        Coincident(line_3_4.end, line_4_4.start, id="c-403")
        Coincident(line_4_4.end, line_1_4.start, id="c-404")
        Horizontal(line_1_4, id="c-405")
        Horizontal(line_3_4, id="c-406")
        Vertical(line_2_4, id="c-407")
        Vertical(line_4_4, id="c-408")
        Distance(line_1_4, 20*mm, id="c-409")
        Equal(line_1_4, line_2_4, id="c-410")
    extrude(
        sketch_4,
        16*mm,
        regions=[Region("e-401,e-402,e-403,e-404", "c=-28,-56|a=400")],
        id="f-ext4",
    )
    fillet(
        [Ref("edge(f-ext4/lateral/e-401|f-ext4/lateral/e-402)", "line|n=0,0,1|c=-18,-66,16|l=16"), Ref("edge(f-ext4/lateral/e-402|f-ext4/lateral/e-403)", "line|n=0,0,1|c=-18,-46,16|l=16"), Ref("edge(f-ext4/lateral/e-403|f-ext4/lateral/e-404)", "line|n=0,0,1|c=-38,-46,16|l=16"), Ref("edge(f-ext4/lateral/e-401|f-ext4/lateral/e-404)", "line|n=0,0,1|c=-38,-66,16|l=16")],
        3*mm,
        id="f-fil4",
    )
    shell(
        Ref("f-ext4/end-cap", "plane|n=0,0,1|c=-28,-56,24|a=392.274334"),
        1.2*mm,
        id="f-sh1",
    )
    linear_pattern(Axis.X, 3, 28*mm, id="f-lp3")
    mirror(Plane.XZ, id="f-mir2")
with base_2:
    boolean(fins, op="join", id="f-bool1")
    boolean(bins, op="join", id="f-bool2")
    with Sketch(Plane.XY, id="f-sk5") as sketch_5:
        line_1_5 = Line((-96, -44), (96, -44), construction=True, id="e-501")
        line_2_5 = Line((96, -44), (96, 44), construction=True, id="e-502")
        line_3_5 = Line((96, 44), (-96, 44), construction=True, id="e-503")
        line_4_5 = Line((-96, 44), (-96, -44), construction=True, id="e-504")
        circle_1_4 = Circle(6, center=(-96, -44), id="e-505")
        circle_2_3 = Circle(6, center=(96, -44), id="e-506")
        circle_3_3 = Circle(6, center=(96, 44), id="e-507")
        circle_4_3 = Circle(6, center=(-96, 44), id="e-508")
        Coincident(line_1_5.end, line_2_5.start, id="c-501")
        Coincident(line_2_5.end, line_3_5.start, id="c-502")
        Coincident(line_3_5.end, line_4_5.start, id="c-503")
        Coincident(line_4_5.end, line_1_5.start, id="c-504")
        Horizontal(line_1_5, id="c-505")
        Horizontal(line_3_5, id="c-506")
        Vertical(line_2_5, id="c-507")
        Vertical(line_4_5, id="c-508")
        Coincident(circle_1_4.center, line_1_5.start, id="c-509")
        Coincident(circle_2_3.center, line_2_5.start, id="c-510")
        Coincident(circle_3_3.center, line_3_5.start, id="c-511")
        Coincident(circle_4_3.center, line_4_5.start, id="c-512")
        Equal(circle_1_4, circle_2_3, id="c-513")
        Equal(circle_2_3, circle_3_3, id="c-514")
        Equal(circle_3_3, circle_4_3, id="c-515")
        Radius(circle_1_4, 6*mm, id="c-516")
    extrude(
        sketch_5,
        1.5*mm,
        regions=[Region("e-505", "c=-96,-44|a=113.097336"), Region("e-506", "c=96,-44|a=113.097336"), Region("e-507", "c=96,44|a=113.097336"), Region("e-508", "c=-96,44|a=113.097336")],
        op="cut",
        id="f-ext5",
    )
    chamfer(
        [Ref("edge(f-ext1/end-cap|f-ext1/lateral/e-101)", "line|n=1,0,0|c=0,-80,0|l=228"), Ref("edge(f-ext1/end-cap|f-ext1/lateral/e-102)", "line|n=0,1,0|c=120,0,0|l=148"), Ref("edge(f-ext1/end-cap|f-ext1/lateral/e-103)", "line|n=1,0,0|c=0,80,0|l=228"), Ref("edge(f-ext1/end-cap|f-ext1/lateral/e-104)", "line|n=0,1,0|c=-120,0,0|l=148")],
        0.5*mm,
        id="f-cha3",
    )
    with Sketch(plane_1, id="f-sk6") as sketch_6:
        line_1_6 = Line((72, -10), (100, -10), id="e-601")
        line_2_6 = Line((100, -10), (100, 10), id="e-602")
        line_3_6 = Line((100, 10), (72, 10), id="e-603")
        line_4_6 = Line((72, 10), (72, -10), id="e-604")
        circle_1_5 = Circle(1.25, center=(78, -16), construction=True, id="e-605")
        circle_2_4 = Circle(1.25, center=(94, -16), construction=True, id="e-606")
        circle_3_4 = Circle(1.25, center=(94, 16), construction=True, id="e-607")
        circle_4_4 = Circle(1.25, center=(78, 16), construction=True, id="e-608")
        Coincident(line_1_6.end, line_2_6.start, id="c-601")
        Coincident(line_2_6.end, line_3_6.start, id="c-602")
        Coincident(line_3_6.end, line_4_6.start, id="c-603")
        Coincident(line_4_6.end, line_1_6.start, id="c-604")
        Horizontal(line_1_6, id="c-605")
        Horizontal(line_3_6, id="c-606")
        Vertical(line_2_6, id="c-607")
        Vertical(line_4_6, id="c-608")
        Distance(line_1_6, 28*mm, id="c-609")
        Distance(line_2_6, 20*mm, id="c-610")
    extrude(
        sketch_6,
        regions=[Region("e-601,e-602,e-603,e-604", "c=86,0|a=560")],
        reverse=True,
        through_all=True,
        op="cut",
        id="f-ext6",
    )
    fillet(
        [Ref("edge(f-ext6/lateral/e-601|f-ext6/lateral/e-602)", "line|n=0,0,1|c=100,-10,4|l=8"), Ref("edge(f-ext6/lateral/e-602|f-ext6/lateral/e-603)", "line|n=0,0,1|c=100,10,4|l=8"), Ref("edge(f-ext6/lateral/e-603|f-ext6/lateral/e-604)", "line|n=0,0,1|c=72,10,4|l=8"), Ref("edge(f-ext6/lateral/e-601|f-ext6/lateral/e-604)", "line|n=0,0,1|c=72,-10,4|l=8")],
        3*mm,
        id="f-fil5",
    )
    chamfer(
        [Ref("edge(f-ext1/start-cap|f-ext6/lateral/e-601)", "line|n=1,0,0|c=86,-10,8|l=22"), Ref("edge(f-ext1/start-cap|f-ext6/lateral/e-602)", "line|n=0,1,0|c=100,0,8|l=14"), Ref("edge(f-ext1/start-cap|f-ext6/lateral/e-603)", "line|n=1,0,0|c=86,10,8|l=22"), Ref("edge(f-ext1/start-cap|f-ext6/lateral/e-604)", "line|n=0,1,0|c=72,0,8|l=14")],
        0.5*mm,
        id="f-cha4",
    )
    hole(
        [circle_1_5.center, circle_2_4.center, circle_3_4.center, circle_4_4.center],
        2.5*mm,
        6*mm,
        tip_angle=118*deg,
        id="f-hole3",
    )
    with Sketch(plane_1, id="f-sk7") as sketch_7:
        line_1_7 = Line((-104, -18), (-68, -18), id="e-701")
        line_2_7 = Line((-68, -18), (-68, 18), id="e-702")
        line_3_7 = Line((-68, 18), (-104, 18), id="e-703")
        line_4_7 = Line((-104, 18), (-104, -18), id="e-704")
        Circle(6, center=(-86, 0), construction=True, id="e-705")
        Coincident(line_1_7.end, line_2_7.start, id="c-701")
        Coincident(line_2_7.end, line_3_7.start, id="c-702")
        Coincident(line_3_7.end, line_4_7.start, id="c-703")
        Coincident(line_4_7.end, line_1_7.start, id="c-704")
        Horizontal(line_1_7, id="c-705")
        Horizontal(line_3_7, id="c-706")
        Vertical(line_2_7, id="c-707")
        Vertical(line_4_7, id="c-708")
        Distance(line_1_7, 36*mm, id="c-709")
        Equal(line_1_7, line_2_7, id="c-710")
    extrude(
        sketch_7,
        3*mm,
        regions=[Region("e-701,e-702,e-703,e-704", "c=-86,0|a=1296")],
        reverse=True,
        op="cut",
        id="f-ext7",
    )
    fillet(
        [Ref("edge(f-ext7/lateral/e-701|f-ext7/lateral/e-702)", "line|n=0,0,1|c=-68,-18,6.5|l=3"), Ref("edge(f-ext7/lateral/e-702|f-ext7/lateral/e-703)", "line|n=0,0,1|c=-68,18,6.5|l=3"), Ref("edge(f-ext7/lateral/e-703|f-ext7/lateral/e-704)", "line|n=0,0,1|c=-104,18,6.5|l=3"), Ref("edge(f-ext7/lateral/e-701|f-ext7/lateral/e-704)", "line|n=0,0,1|c=-104,-18,6.5|l=3")],
        4*mm,
        id="f-fil6",
    )
    chamfer(
        [Ref("edge(f-ext1/start-cap|f-ext7/lateral/e-701)", "line|n=1,0,0|c=-86,-18,8|l=28"), Ref("edge(f-ext1/start-cap|f-ext7/lateral/e-702)", "line|n=0,1,0|c=-68,0,8|l=28"), Ref("edge(f-ext1/start-cap|f-ext7/lateral/e-703)", "line|n=1,0,0|c=-86,18,8|l=28"), Ref("edge(f-ext1/start-cap|f-ext7/lateral/e-704)", "line|n=0,1,0|c=-104,0,8|l=28")],
        0.5*mm,
        id="f-cha5",
    )
    with Sketch(plane_1, id="f-sk8") as sketch_8:
        line_1_8 = Line((72, 24), (104, 24), id="e-801")
        line_2_8 = Line((104, 24), (104, 56), id="e-802")
        line_3_8 = Line((104, 56), (72, 56), id="e-803")
        line_4_8 = Line((72, 56), (72, 24), id="e-804")
        Circle(8, center=(88, 40), construction=True, id="e-805")
        Coincident(line_1_8.end, line_2_8.start, id="c-801")
        Coincident(line_2_8.end, line_3_8.start, id="c-802")
        Coincident(line_3_8.end, line_4_8.start, id="c-803")
        Coincident(line_4_8.end, line_1_8.start, id="c-804")
        Horizontal(line_1_8, id="c-805")
        Horizontal(line_3_8, id="c-806")
        Vertical(line_2_8, id="c-807")
        Vertical(line_4_8, id="c-808")
        Distance(line_1_8, 32*mm, id="c-809")
        Equal(line_1_8, line_2_8, id="c-810")
    extrude(
        sketch_8,
        16*mm,
        regions=[Region("e-801,e-802,e-803,e-804", "c=88,40|a=1024")],
        id="f-ext8",
    )
    fillet(
        [Ref("edge(f-ext8/lateral/e-801|f-ext8/lateral/e-802)", "line|n=0,0,1|c=104,24,16|l=16"), Ref("edge(f-ext8/lateral/e-802|f-ext8/lateral/e-803)", "line|n=0,0,1|c=104,56,16|l=16"), Ref("edge(f-ext8/lateral/e-803|f-ext8/lateral/e-804)", "line|n=0,0,1|c=72,56,16|l=16"), Ref("edge(f-ext8/lateral/e-801|f-ext8/lateral/e-804)", "line|n=0,0,1|c=72,24,16|l=16")],
        4*mm,
        id="f-fil7",
    )
    with Sketch(plane_1, id="f-sk9") as sketch_9:
        line_1_9 = Line((-104, 24), (-72, 24), id="e-901")
        line_2_9 = Line((-72, 24), (-72, 56), id="e-902")
        line_3_9 = Line((-72, 56), (-104, 56), id="e-903")
        line_4_9 = Line((-104, 56), (-104, 24), id="e-904")
        circle_1_6 = Circle(3, center=(-96, 32), construction=True, id="e-905")
        circle_2_5 = Circle(3, center=(-80, 32), construction=True, id="e-906")
        circle_3_5 = Circle(3, center=(-80, 48), construction=True, id="e-907")
        circle_4_5 = Circle(3, center=(-96, 48), construction=True, id="e-908")
        Coincident(line_1_9.end, line_2_9.start, id="c-901")
        Coincident(line_2_9.end, line_3_9.start, id="c-902")
        Coincident(line_3_9.end, line_4_9.start, id="c-903")
        Coincident(line_4_9.end, line_1_9.start, id="c-904")
        Horizontal(line_1_9, id="c-905")
        Horizontal(line_3_9, id="c-906")
        Vertical(line_2_9, id="c-907")
        Vertical(line_4_9, id="c-908")
        Distance(line_1_9, 32*mm, id="c-909")
        Equal(line_1_9, line_2_9, id="c-910")
    extrude(
        sketch_9,
        12*mm,
        regions=[Region("e-901,e-902,e-903,e-904", "c=-88,40|a=1024")],
        id="f-ext9",
    )
    hole(
        [circle_1_6.center, circle_2_5.center, circle_3_5.center, circle_4_5.center],
        6*mm,
        reverse=True,
        id="f-hole4",
    )
    chamfer(
        [Ref("edge(f-ext9/end-cap|f-ext9/lateral/e-901)", "line|n=1,0,0|c=-88,24,20|l=32"), Ref("edge(f-ext9/end-cap|f-ext9/lateral/e-902)", "line|n=0,1,0|c=-72,40,20|l=32"), Ref("edge(f-ext9/end-cap|f-ext9/lateral/e-903)", "line|n=1,0,0|c=-88,56,20|l=32"), Ref("edge(f-ext9/end-cap|f-ext9/lateral/e-904)", "line|n=0,1,0|c=-104,40,20|l=32")],
        1*mm,
        id="f-cha6",
    )
with lids:
    with Sketch(Plane.XY, id="f-sk10") as sketch_10:
        line_1_10 = Line((-40, -122), (-16, -122), id="e-1001")
        line_2_10 = Line((-16, -122), (-16, -98), id="e-1002")
        line_3_10 = Line((-16, -98), (-40, -98), id="e-1003")
        line_4_10 = Line((-40, -98), (-40, -122), id="e-1004")
        Circle(3, center=(-28, -110), construction=True, id="e-1005")
        Coincident(line_1_10.end, line_2_10.start, id="c-1001")
        Coincident(line_2_10.end, line_3_10.start, id="c-1002")
        Coincident(line_3_10.end, line_4_10.start, id="c-1003")
        Coincident(line_4_10.end, line_1_10.start, id="c-1004")
        Horizontal(line_1_10, id="c-1005")
        Horizontal(line_3_10, id="c-1006")
        Vertical(line_2_10, id="c-1007")
        Vertical(line_4_10, id="c-1008")
        Distance(line_1_10, lid_width, id="c-1009")
        Equal(line_1_10, line_2_10, id="c-1010")
    extrude(
        sketch_10,
        6*mm,
        regions=[Region("e-1001,e-1002,e-1003,e-1004", "c=-28,-110|a=576")],
        id="f-ext10",
    )
    fillet(
        [Ref("edge(f-ext10/lateral/e-1001|f-ext10/lateral/e-1002)", "line|n=0,0,1|c=-16,-122,3|l=6"), Ref("edge(f-ext10/lateral/e-1002|f-ext10/lateral/e-1003)", "line|n=0,0,1|c=-16,-98,3|l=6"), Ref("edge(f-ext10/lateral/e-1003|f-ext10/lateral/e-1004)", "line|n=0,0,1|c=-40,-98,3|l=6"), Ref("edge(f-ext10/lateral/e-1001|f-ext10/lateral/e-1004)", "line|n=0,0,1|c=-40,-122,3|l=6")],
        3*mm,
        id="f-fil8",
    )
    shell(
        Ref("f-ext10/start-cap", "plane|n=0,0,-1|c=-28,-110,0|a=568.274334"),
        1.2*mm,
        id="f-sh2",
    )
    linear_pattern(Axis.X, 3, 28*mm, id="f-lp4")
