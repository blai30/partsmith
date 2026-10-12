"""Unit tests of the frame codec in partsmith._frames."""

import json

import pytest
from support import EXAMPLE_A

from partsmith import PartsmithError
from partsmith._frames import aligned, decode_response, encode_request

EXAMPLE_B_HEADER = (
    '{"protocol_version":1,"messages":[{"kind":"request","id":2,"command":'
    '{"type":"import_step","doc":1,"data":0,"name":"Bracket"}}],'
    '"buffers":[{"offset":0,"length":14,"element_type":"u8"}]}'
)
"""The header of example B of protocol.md section 2.2."""


def response_frame(messages: list[dict], buffers: list[bytes] = (), protocol_version: int = 1) -> bytes:
    """A response frame with these messages and u8 buffers, laid out by hand."""
    table = []
    offset = 0
    for data in buffers:
        table.append({"offset": offset, "length": len(data), "element_type": "u8"})
        offset = aligned(offset + len(data))
    header = json.dumps({"protocol_version": protocol_version, "messages": messages, "buffers": table}).encode()
    frame = b"PSM1" + len(header).to_bytes(4, "little") + header + bytes(aligned(len(header)) - len(header))
    for entry, data in zip(table, buffers, strict=True):
        frame += bytes(8 + aligned(len(header)) + entry["offset"] - len(frame)) + data
    return frame


def ok_response(request_id: int = 1) -> dict:
    """A successful engine_info response message."""
    return {"kind": "response", "id": request_id, "result": {"kind": "ok", "type": "engine_info"}}


@pytest.mark.parametrize(("length", "expected"), [(0, 0), (1, 8), (8, 8), (107, 112), (185, 192)])
def test_aligned_rounds_up_to_a_multiple_of_8(length, expected):
    assert aligned(length) == expected


def test_a_request_without_buffers_is_example_a():
    assert encode_request(1, {"type": "engine_info"}) == EXAMPLE_A


def test_a_request_with_one_buffer_is_example_b():
    command = {"type": "import_step", "doc": 1, "data": 0, "name": "Bracket"}

    frame = encode_request(2, command, [b"ISO-10303-21;\n"])

    assert len(frame) == 214
    assert frame[8 : 8 + 185].decode() == EXAMPLE_B_HEADER
    assert frame[200:] == b"ISO-10303-21;\n"


def test_a_second_buffer_starts_at_the_next_multiple_of_8():
    frame = encode_request(3, {"type": "engine_info"}, [bytes(14), b"\x01" * 12])

    header = json.loads(frame[8 : 8 + int.from_bytes(frame[4:8], "little")])

    assert [entry["offset"] for entry in header["buffers"]] == [0, 16]
    assert frame.endswith(b"\x01" * 12)


def test_a_request_with_a_non_finite_number_is_refused():
    with pytest.raises(ValueError):
        encode_request(1, {"type": "evaluate", "value": float("nan")})


def test_decoding_splits_events_from_the_response():
    event = {"kind": "event", "type": "document_changed", "doc": 1}

    decoded = decode_response(response_frame([event, ok_response(7)]))

    assert decoded.events == [event]
    assert decoded.response["id"] == 7
    assert decoded.buffers == []


def test_decoding_reads_each_buffer():
    decoded = decode_response(response_frame([ok_response()], [b"abc", b"0123456789"]))

    assert decoded.buffers == [b"abc", b"0123456789"]


def test_decoding_rejects_a_frame_without_the_magic():
    with pytest.raises(PartsmithError, match="does not start with PSM1"):
        decode_response(b"hello")


def test_decoding_rejects_a_cut_off_header():
    with pytest.raises(PartsmithError, match="header is cut off"):
        decode_response(response_frame([ok_response()])[:12])


def test_decoding_rejects_another_protocol_version():
    with pytest.raises(PartsmithError, match="protocol version 2"):
        decode_response(response_frame([ok_response()], protocol_version=2))


def test_decoding_rejects_a_frame_without_a_response():
    with pytest.raises(PartsmithError, match="no response message"):
        decode_response(response_frame([{"kind": "event", "type": "recomputed"}]))


def test_decoding_rejects_a_buffer_past_the_end():
    frame = response_frame([ok_response()], [b"abcdefgh"])

    with pytest.raises(PartsmithError, match="runs past its end"):
        decode_response(frame[:-1])
