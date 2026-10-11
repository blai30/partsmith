//! The cxx bridge to the C++ shim. Every function returns `Result`, so C++ exceptions become errors.

#[cxx::bridge(namespace = "partsmith::occt")]
pub(crate) mod ffi {
    unsafe extern "C++" {
        include!("partsmith-occt-sys/cpp/shim.h");

        type Shape;

        fn make_box(dx: f64, dy: f64, dz: f64) -> Result<SharedPtr<Shape>>;
        fn shape_volume(shape: &Shape) -> Result<f64>;
    }
}
