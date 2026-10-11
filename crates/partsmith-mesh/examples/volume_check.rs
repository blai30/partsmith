//! Temporary micro-check (not committed).
use std::time::Instant;
use partsmith_mesh::Mesh;

fn main() {
    let segments = 1000u32; let rings = 501u32; let radius = 50.0;
    let mut positions = vec![[0.0, 0.0, -radius]];
    for ring in 1..rings { let polar = std::f64::consts::PI * f64::from(ring) / f64::from(rings);
        for segment in 0..segments { let azimuth = std::f64::consts::TAU * f64::from(segment) / f64::from(segments);
            positions.push([radius * polar.sin() * azimuth.cos(), radius * polar.sin() * azimuth.sin(), -radius * polar.cos()]); } }
    positions.push([0.0, 0.0, radius]);
    let top = (positions.len() - 1) as u32;
    let vertex = |ring: u32, segment: u32| 1 + (ring - 1) * segments + segment % segments;
    let mut triangles = Vec::new();
    for segment in 0..segments { triangles.push([0, vertex(1, segment + 1), vertex(1, segment)]); }
    for ring in 1..rings - 1 { for segment in 0..segments {
        triangles.push([vertex(ring, segment), vertex(ring, segment + 1), vertex(ring + 1, segment + 1)]);
        triangles.push([vertex(ring, segment), vertex(ring + 1, segment + 1), vertex(ring + 1, segment)]); }}
    for segment in 0..segments { triangles.push([top, vertex(rings - 1, segment), vertex(rings - 1, segment + 1)]); }
    let mesh = Mesh::from_triangles(positions, triangles).unwrap();
    let cross = |a: [f64; 3], b: [f64; 3]| [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
    let sub = |a: [f64; 3], b: [f64; 3]| [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
    let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
    let term = |t: &[u32; 3]| { let p = t.map(|v| mesh.positions()[v as usize]); dot(p[0], cross(sub(p[1], p[0]), sub(p[2], p[0]))) / 6.0 };
    for _ in 0..3 {
        let start = Instant::now();
        let plain: f64 = mesh.triangles().iter().map(term).sum();
        let plain_time = start.elapsed();
        let start = Instant::now();
        let (mut total, mut compensation) = (0.0f64, 0.0f64);
        for t in mesh.triangles() { let value = term(t); let next = total + value;
            if total.abs() >= value.abs() { compensation += (total - next) + value } else { compensation += (value - next) + total } total = next; }
        let neumaier_time = start.elapsed();
        println!("plain {plain_time:?} neumaier {neumaier_time:?} {plain} {}", total + compensation);
    }
    for _ in 0..3 {
        let start = Instant::now(); let volume = mesh.signed_volume(); let volume_time = start.elapsed();
        let start = Instant::now(); let area = mesh.area(); let area_time = start.elapsed();
        let start = Instant::now(); let center = mesh.center_of_mass(); let center_time = start.elapsed();
        let start = Instant::now(); let bounds = mesh.bounding_box(); let bounds_time = start.elapsed();
        println!("volume {volume_time:?} area {area_time:?} center {center_time:?} bounds {bounds_time:?} {volume} {area} {center:?} {bounds:?}");
    }
}
