"""Extension module and packaging acceptance tests (python-api.md section 6, tests 72 to 77)."""

import re
import shutil
import subprocess
import sys
from concurrent.futures import ThreadPoolExecutor

import partsmith
import pytest
from partsmith import _core
from support import EXAMPLE_A, REPOSITORY_ROOT, SCHEMAS, request_frame, response_message

CALLS_PER_THREAD = 50
"""Calls per thread in test 74."""

EXPECTED_ALL = [
    # documents and values
    "Document", "current_document", "param", "Param", "rollback", "expr", "Expr",
    "mm", "cm", "meter", "inch", "deg", "rad", "printer",
    # parts
    "Part",
    # sketches
    "Sketch", "Plane", "Placement", "Region", "Point", "Line", "Circle", "Arc", "Rectangle", "Polygon",
    "Coincident", "Horizontal", "Vertical", "Parallel", "Perpendicular", "Tangent", "Collinear", "Equal",
    "Concentric", "Midpoint", "PointOn", "Symmetric", "Fix", "Distance", "HorizontalDistance",
    "VerticalDistance", "Radius", "Diameter", "Angle",
    # features
    "Feature", "DatumPlane", "extrude", "revolve", "fillet", "chamfer", "shell", "hole",
    "box", "cylinder", "sphere", "cone", "torus", "boolean", "mirror", "linear_pattern",
    "circular_pattern", "datum_plane", "move", "split", "feature",
    # selectors
    "Axis", "Element", "Selection", "Ref", "faces", "edges", "vertices",
    # export and reports
    "export_stl", "export_3mf", "export_obj", "export_step", "BoundingBox", "PrintReport", "Finding",
    # running
    "run_script", "run_code",
    # errors
    "PartsmithError", "EngineError", "FeatureError", "SelectionError", "ExpressionError", "PartsmithWarning",
]
"""The public names of python-api.md section 2.3, in order."""


def test_t072_version_constants():
    assert _core.PROTOCOL_VERSION == 1
    assert _core.ENGINE_VERSION == "0.1.0"
    assert _core.FORMAT_VERSION == 1


def test_t073_example_a_returns_an_engine_info_response_with_id_1():
    response = response_message(_core.Session().call(EXAMPLE_A))

    assert response["id"] == 1
    assert response["result"]["kind"] == "ok"
    assert response["result"]["type"] == "engine_info"


def test_t073_garbage_bytes_return_a_malformed_frame_error_without_raising():
    response = response_message(_core.Session().call(b"hello"))

    assert response["result"]["kind"] == "error"
    assert response["result"]["code"] == "malformed_frame"


def test_t073_a_str_frame_raises_type_error():
    session = _core.Session()

    with pytest.raises(TypeError):
        session.call("text")


def test_t074_two_threads_share_one_session():
    session = _core.Session()

    def send(thread_index: int) -> list[tuple[int, dict]]:
        responses = []
        for call_index in range(1, CALLS_PER_THREAD + 1):
            request_id = thread_index * 1000 + call_index
            frame = session.call(request_frame(request_id, {"type": "engine_info"}))
            responses.append((request_id, response_message(frame)))
        return responses

    with ThreadPoolExecutor(max_workers=2) as executor:
        results = list(executor.map(send, [0, 1]))

    assert [len(responses) for responses in results] == [CALLS_PER_THREAD, CALLS_PER_THREAD]
    for responses in results:
        for request_id, response in responses:
            assert response["id"] == request_id
            assert response["result"]["kind"] == "ok"
            assert response["result"]["type"] == "engine_info"


def test_t075_protocol_schema_matches_the_checked_in_file():
    expected = (SCHEMAS / "protocol.schema.json").read_bytes().decode("utf-8")

    assert _core.protocol_schema() == expected


def test_t075_ptsm_schema_matches_the_checked_in_file():
    expected = (SCHEMAS / "ptsm.schema.json").read_bytes().decode("utf-8")

    assert _core.ptsm_schema() == expected


@pytest.fixture(scope="module")
def release_wheels(tmp_path_factory):
    """Runs `maturin build --release` in python/ into a fresh directory and returns that directory."""
    wheels = tmp_path_factory.mktemp("wheels")
    subprocess.run(
        [sys.executable, "-m", "maturin", "build", "--release", "--out", str(wheels)],
        cwd=REPOSITORY_ROOT / "python",
        check=True,
    )
    return wheels


@pytest.mark.packaging
def test_t076_release_build_produces_one_abi3_wheel(release_wheels):
    built = sorted(path.name for path in release_wheels.iterdir())

    assert len(built) == 1
    assert re.fullmatch(r"partsmith-0\.1\.0-cp312-abi3-[A-Za-z0-9_.]+\.whl", built[0])


@pytest.mark.packaging
@pytest.mark.parametrize("interpreter", ["python3.12", "python3.13"])
def test_t076_release_wheel_installs_and_imports(interpreter, release_wheels, tmp_path):
    executable = shutil.which(interpreter)
    assert executable is not None, f"{interpreter} is required to check the wheel"
    [wheel] = list(release_wheels.iterdir())
    environment = tmp_path / "environment"
    subprocess.run([executable, "-m", "venv", str(environment)], check=True)
    environment_python = environment / "bin" / "python"
    subprocess.run(
        [str(environment_python), "-m", "pip", "install", "--no-index", str(wheel)],
        check=True,
    )

    imported = subprocess.run(
        [str(environment_python), "-c", "import partsmith, partsmith._core; print(partsmith._core.ENGINE_VERSION)"],
        cwd=tmp_path,
        capture_output=True,
        text=True,
        check=False,
    )

    assert imported.returncode == 0, imported.stderr
    assert imported.stdout == "0.1.0\n"


def test_t077_all_lists_exactly_the_public_names():
    assert partsmith.__all__ == EXPECTED_ALL


def test_t077_every_public_name_is_importable():
    missing = [name for name in EXPECTED_ALL if not hasattr(partsmith, name)]

    assert missing == []
