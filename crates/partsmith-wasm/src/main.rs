//! The web core module `partsmith_core`: an Emscripten executable whose runtime stays alive after `main` returns, so JavaScript can call `ps_alloc`, `ps_free` and `ps_call` (protocol.md section 2.10, ADR 0009).

fn main() {
    // Creating the session here pays its setup cost while the module is instantiated. Calling into the library also keeps it, and with it the three exports, in the link.
    partsmith_wasm::initialize_session();
}
