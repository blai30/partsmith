mod common;
use common::generator::generate;
use partsmith_sketch::*;

#[test]
fn debug_idempotence() {
    let case = generate(10715863859186601374).unwrap();
    for e in &case.warm_start.entities { println!("{}", serde_json::to_string(e).unwrap()); }
    for c in &case.warm_start.constraints { println!("{}", serde_json::to_string(c).unwrap()); }
    println!("{:?}", case.values);
    let first = NativeSolver.solve(&case.warm_start, &case.values).unwrap();
    println!("first failed {} dof {} red {:?} conf {:?}", first.failed, first.dof, first.redundant, first.conflicting);
    for e in &first.entities { println!("{}", serde_json::to_string(e).unwrap()); }
    let mut again = case.warm_start.clone();
    again.entities = first.entities.clone();
    let second = NativeSolver.solve(&again, &case.values).unwrap();
    println!("second failed {}", second.failed);
    for (a, b) in first.entities.iter().zip(&second.entities) {
        if format!("{a:?}") != format!("{b:?}") { println!("DIFF\n{a:?}\n{b:?}"); }
    }
}
