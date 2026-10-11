//! Temporary performance sanity check (not committed).
use std::time::Instant;

use partsmith_mesh::{Bvh, Mesh, Ray, WELD_TOLERANCE};

fn sphere(radius: f64, segments: u32, rings: u32) -> Mesh {
    let mut positions = vec![[0.0, 0.0, -radius]];
    for ring in 1..rings {
        let polar = std::f64::consts::PI * f64::from(ring) / f64::from(rings);
        for segment in 0..segments {
            let azimuth = std::f64::consts::TAU * f64::from(segment) / f64::from(segments);
            positions.push([radius * polar.sin() * azimuth.cos(), radius * polar.sin() * azimuth.sin(), -radius * polar.cos()]);
        }
    }
    positions.push([0.0, 0.0, radius]);
    let top = (positions.len() - 1) as u32;
    let vertex = |ring: u32, segment: u32| 1 + (ring - 1) * segments + segment % segments;
    let mut triangles = Vec::new();
    for segment in 0..segments { triangles.push([0, vertex(1, segment + 1), vertex(1, segment)]); }
    for ring in 1..rings - 1 { for segment in 0..segments {
        triangles.push([vertex(ring, segment), vertex(ring, segment + 1), vertex(ring + 1, segment + 1)]);
        triangles.push([vertex(ring, segment), vertex(ring + 1, segment + 1), vertex(ring + 1, segment)]);
    }}
    for segment in 0..segments { triangles.push([top, vertex(rings - 1, segment), vertex(rings - 1, segment + 1)]); }
    Mesh::from_triangles(positions, triangles).unwrap()
}

fn time<T>(label: &str, run: impl Fn() -> T) -> T {
    let start = Instant::now();
    let first = run();
    let cold = start.elapsed().as_secs_f64() * 1000.0;
    drop(first);
    let start = Instant::now();
    let result = run();
    println!("{label}: cold {cold:.1} ms, warm {:.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    result
}

fn main() {
    let shared = sphere(50.0, 1000, 501);
    println!("triangles {}", shared.triangle_count());
    let positions: Vec<_> = shared.triangles().iter().flat_map(|t| t.map(|v| shared.positions()[v as usize])).collect();
    let triangles: Vec<_> = (0..shared.triangle_count() as u32).map(|t| [3 * t, 3 * t + 1, 3 * t + 2]).collect();
    let face_ids: Vec<u32> = (0..shared.triangle_count() as u32).map(|t| t / 200).collect();
    let faceted = time("new (3M vertices)", || Mesh::new(positions.clone(), triangles.clone(), face_ids.clone(), None, vec![], vec![]).unwrap());
    let welded = time("weld", || faceted.weld(WELD_TOLERANCE).unwrap());
    println!("welded vertices {}", welded.mesh.vertex_count());
    let mesh = welded.mesh;
    time("signed_volume", || mesh.signed_volume());
    time("area", || mesh.area());
    time("bounding_box", || mesh.bounding_box());
    time("center_of_mass", || mesh.center_of_mass());
    let report = time("validity", || mesh.validity());
    println!("valid {}", report.is_valid());
    let shells = time("shells", || mesh.shells());
    println!("shells {}", shells.len());
    time("render_buffers", || faceted.render_buffers());
    let bvh = time("bvh", || Bvh::new(&mesh));
    let mut state = 12345_u64;
    let mut random = || { state ^= state << 13; state ^= state >> 7; state ^= state << 17; (state >> 11) as f64 / (1u64 << 53) as f64 };
    let rays: Vec<Ray> = (0..100_000).map(|_| Ray::new([random() * 40.0 - 20.0, random() * 40.0 - 20.0, random() * 40.0 - 20.0], [random() - 0.5, random() - 0.5, random() - 0.5]).unwrap()).collect();
    let start = Instant::now();
    let hits = rays.iter().filter(|ray| bvh.first_hit(ray, 0.0, f64::INFINITY).is_some()).count();
    println!("first_hit mean: {:.2} us (hits {hits})", start.elapsed().as_secs_f64() * 1e6 / rays.len() as f64);
    let small = sphere(50.0, 400, 251);
    let small = small.weld(WELD_TOLERANCE).unwrap().mesh;
    println!("slice mesh triangles {}", small.triangle_count());
    let heights: Vec<f64> = (0..1000).map(|i| -49.95 + 0.1 * f64::from(i)).collect();
    let slices = time("slice_many 1000", || small.slice_many(&heights).unwrap());
    println!("loops ok {}", slices.iter().all(|s| s.loops.len() == 1 && s.open_chains.is_empty()));
}
