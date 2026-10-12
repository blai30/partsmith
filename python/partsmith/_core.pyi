"""Stubs for the `partsmith._core` extension module (python-api.md section 2.2)."""

PROTOCOL_VERSION: int
"""The protocol version of the frames `Session.call` takes and returns."""
ENGINE_VERSION: str
"""The engine version."""
FORMAT_VERSION: int
"""The newest `.ptsm` format version the engine reads and writes."""

class Session:
    """One engine session. Thread-safe: concurrent calls are serialized."""

    def __init__(self, *, display: bool = False, deterministic_ids: bool = True) -> None: ...
    def call(self, frame: bytes) -> bytes:
        """Handles one request frame and returns the response frame. Engine errors are inside the frame."""

def ptsm_schema() -> str:
    """The .ptsm JSON Schema text (engine ptsm_schema)."""

def protocol_schema() -> str:
    """The protocol JSON Schema text (engine protocol_schema)."""
