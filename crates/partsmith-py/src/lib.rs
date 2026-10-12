//! The `partsmith._core` extension module (python-api.md section 2.2).
//!
//! It exposes one engine [`Session`] per Python `Session` object, which takes and returns
//! protocol frames, plus the version constants and the generated JSON Schema texts. Everything
//! else lives in the pure-Python `partsmith` package, which is a protocol client on top of
//! `Session.call`.

use std::sync::{Mutex, MutexGuard, PoisonError};

use partsmith_engine::{ENGINE_VERSION, SequentialIds, Session, SessionOptions};
use partsmith_model::{FORMAT_VERSION, IdGenerator, RandomIdGenerator};
use partsmith_protocol::PROTOCOL_VERSION;
use pyo3::prelude::*;
use pyo3::types::PyBytes;

/// A Python-owned engine session.
///
/// It is thread-safe: concurrent calls from several Python threads are serialized by the mutex,
/// and each call releases the GIL while the engine runs.
#[pyclass(name = "Session", module = "partsmith._core", frozen)]
pub struct PySession {
    /// The engine session behind every call.
    session: Mutex<Session>,
}

#[pymethods]
impl PySession {
    /// Creates a session; `deterministic_ids` selects `SequentialIds`, otherwise ids are random.
    #[new]
    #[pyo3(signature = (*, display = false, deterministic_ids = true))]
    fn new(display: bool, deterministic_ids: bool) -> Self {
        PySession {
            session: Mutex::new(Session::new(session_options(display, deterministic_ids))),
        }
    }

    /// Handles one request frame and returns the response frame. Engine errors are inside the
    /// frame; only a `frame` that is not `bytes` raises (`TypeError`).
    fn call<'py>(&self, py: Python<'py>, frame: &[u8]) -> Bound<'py, PyBytes> {
        let request = frame.to_vec();
        let response = py.detach(|| self.lock().call(&request));
        PyBytes::new(py, &response)
    }
}

impl PySession {
    /// The engine session, locked for one call.
    fn lock(&self) -> MutexGuard<'_, Session> {
        // `Session::call` contains every command's panic and poisons only that command's
        // document (engine.md section 3.15), so the session stays consistent and a poisoned
        // lock carries no broken state.
        self.session.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// The options of a Python session: `SessionOptions::headless()` with the display flag and the
/// id generator chosen by the caller.
fn session_options(display: bool, deterministic_ids: bool) -> SessionOptions {
    let id_generator: Box<dyn IdGenerator + Send> = if deterministic_ids {
        Box::new(SequentialIds::new())
    } else {
        Box::new(RandomIdGenerator::new())
    };
    SessionOptions {
        display,
        id_generator,
        ..SessionOptions::headless()
    }
}

/// The `.ptsm` JSON Schema text (engine `ptsm_schema`).
#[pyfunction]
fn ptsm_schema() -> String {
    partsmith_engine::ptsm_schema()
}

/// The protocol JSON Schema text (engine `protocol_schema`).
#[pyfunction]
fn protocol_schema() -> String {
    partsmith_engine::protocol_schema()
}

/// The `partsmith._core` extension module.
#[pymodule]
fn _core(module: &Bound<'_, PyModule>) -> PyResult<()> {
    module.add("PROTOCOL_VERSION", PROTOCOL_VERSION)?;
    module.add("ENGINE_VERSION", ENGINE_VERSION)?;
    module.add("FORMAT_VERSION", FORMAT_VERSION)?;
    module.add_class::<PySession>()?;
    module.add_function(wrap_pyfunction!(ptsm_schema, module)?)?;
    module.add_function(wrap_pyfunction!(protocol_schema, module)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::thread;
    use std::time::Duration;

    use partsmith_model::{Id, IdKind};
    use partsmith_protocol::{Command, Request, decode_response_frame, encode_request_frame};
    use pyo3::exceptions::PyTypeError;
    use pyo3::types::PyDict;

    use super::*;

    /// How long a thread may wait for the GIL before the test counts it as held for good.
    const GIL_WAIT_LIMIT: Duration = Duration::from_secs(10);

    /// The first body id the options' generator produces for an empty document.
    fn first_body_id(options: &mut SessionOptions) -> String {
        options
            .id_generator
            .generate(IdKind::Body, &|_: &Id| false)
            .expect("an empty document has every id free")
            .to_string()
    }

    /// A request frame holding one `engine_info` request with `id` and no buffers, built with the
    /// protocol encoder.
    fn engine_info_frame(id: u64) -> Vec<u8> {
        let request = Request {
            id,
            command: Command::EngineInfo,
        };
        encode_request_frame(&request, &[]).expect("a request without buffers always encodes")
    }

    /// The `partsmith._core` module built in an embedded interpreter.
    fn core_module(py: Python<'_>) -> Bound<'_, PyModule> {
        let module = PyModule::new(py, "_core").expect("a module object is created");
        _core(&module).expect("the module initializes");
        module
    }

    #[test]
    fn options_keep_the_display_flag() {
        assert!(session_options(true, true).display);
        assert!(!session_options(false, true).display);
    }

    #[test]
    fn deterministic_options_count_ids() {
        let mut options = session_options(false, true);

        assert_eq!(first_body_id(&mut options), "b-000000000001");
    }

    #[test]
    fn random_options_do_not_count_ids() {
        let mut options = session_options(false, false);

        let id = first_body_id(&mut options);

        assert_ne!(id, "b-000000000001");
        assert_eq!(id.len(), "b-".len() + 12);
    }

    #[test]
    fn options_keep_the_headless_cache_and_undo_settings() {
        let headless = SessionOptions::headless();

        let options = session_options(true, false);

        assert_eq!(options.cache_budget, headless.cache_budget);
        assert_eq!(options.undo_limit, headless.undo_limit);
        assert_eq!(options.tessellation, headless.tessellation);
    }

    #[test]
    fn call_returns_the_engine_response_frame() {
        Python::initialize();
        let session = PySession::new(false, true);

        let response =
            Python::attach(|py| session.call(py, &engine_info_frame(7)).as_bytes().to_vec());

        let decoded = decode_response_frame(&response).expect("the frame decodes");
        assert_eq!(decoded.response.id, 7);
    }

    #[test]
    fn call_releases_the_gil_while_it_waits_for_the_session() {
        Python::initialize();
        let session = PySession::new(false, true);
        let frame = engine_info_frame(9);
        let (attached_sender, attached_receiver) = mpsc::channel();
        let (probe_sender, probe_receiver) = mpsc::channel();

        let guard = session.lock();
        let (probe, caller_was_waiting, response) = thread::scope(|scope| {
            let caller = scope.spawn(|| {
                Python::attach(|py| {
                    attached_sender
                        .send(())
                        .expect("the test thread keeps the receiver until the scope ends");
                    session.call(py, &frame).as_bytes().to_vec()
                })
            });
            attached_receiver
                .recv()
                .expect("the caller thread attaches to Python before it calls");
            // The caller now holds the GIL until `call` releases it, so a second thread can
            // attach only if `call` waits for the session lock with the GIL released.
            let prober = scope.spawn(move || {
                Python::attach(|_| {
                    probe_sender
                        .send(())
                        .expect("the test thread keeps the receiver until the scope ends");
                });
            });
            let probe = probe_receiver.recv_timeout(GIL_WAIT_LIMIT);
            let caller_was_waiting = !caller.is_finished();
            drop(guard);
            prober.join().expect("the prober thread finished");
            let response = caller.join().expect("the caller thread finished");
            (probe, caller_was_waiting, response)
        });

        assert!(
            probe.is_ok(),
            "another thread could not attach to Python while a call waited for the session"
        );
        assert!(
            caller_was_waiting,
            "the call finished before the session was unlocked"
        );
        let decoded = decode_response_frame(&response).expect("the frame decodes");
        assert_eq!(decoded.response.id, 9);
    }

    #[test]
    fn module_exports_the_version_constants() {
        Python::initialize();
        Python::attach(|py| {
            let module = core_module(py);
            let constant = |name: &str| module.getattr(name).expect("the constant exists");

            let protocol_version: u32 = constant("PROTOCOL_VERSION")
                .extract()
                .expect("PROTOCOL_VERSION is an int");
            let engine_version: String = constant("ENGINE_VERSION")
                .extract()
                .expect("ENGINE_VERSION is a str");
            let format_version: u32 = constant("FORMAT_VERSION")
                .extract()
                .expect("FORMAT_VERSION is an int");

            assert_eq!(protocol_version, PROTOCOL_VERSION);
            assert_eq!(engine_version, ENGINE_VERSION);
            assert_eq!(format_version, FORMAT_VERSION);
        });
    }

    #[test]
    fn module_exports_the_schema_texts() {
        Python::initialize();
        Python::attach(|py| {
            let module = core_module(py);
            let text = |name: &str| -> String {
                module
                    .getattr(name)
                    .and_then(|function| function.call0())
                    .and_then(|text| text.extract())
                    .expect("the schema function returns a str")
            };

            assert_eq!(text("ptsm_schema"), partsmith_engine::ptsm_schema());
            assert_eq!(text("protocol_schema"), partsmith_engine::protocol_schema());
        });
    }

    #[test]
    fn session_options_are_keyword_only() {
        Python::initialize();
        Python::attach(|py| {
            let session_type = py.get_type::<PySession>();
            let keywords = PyDict::new(py);
            keywords
                .set_item("display", true)
                .expect("a dict takes a str key");
            keywords
                .set_item("deterministic_ids", false)
                .expect("a dict takes a str key");

            let positional = session_type
                .call1((true,))
                .expect_err("a positional option is rejected");

            assert!(
                positional.is_instance_of::<PyTypeError>(py),
                "expected TypeError, got {positional}"
            );
            assert!(session_type.call((), Some(&keywords)).is_ok());
        });
    }
}
