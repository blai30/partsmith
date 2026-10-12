"""The protocol frame codec (protocol.md section 2.2).

A frame is the magic `PSM1`, a little-endian `u32` header length, the compact UTF-8 JSON header,
zero padding to a multiple of 8, then the buffer section. Every buffer starts at a multiple of 8
from the start of the buffer section. Python only sends and receives `u8` buffers (file bytes).
"""

import json
from collections.abc import Sequence
from dataclasses import dataclass

from .errors import PartsmithError

MAGIC = b"PSM1"
"""The first four bytes of every frame."""

PROTOCOL_VERSION = 1
"""The protocol version this codec writes and reads."""

ALIGNMENT = 8
"""The header and every buffer are padded to a multiple of this many bytes."""

PREFIX_LENGTH = 8
"""The magic plus the header length field."""


@dataclass(frozen=True)
class Frame:
    """A decoded response frame: its event messages, its response message and its buffers."""

    events: list[dict]
    response: dict
    buffers: list[bytes]


def aligned(length: int) -> int:
    """The smallest multiple of 8 that is at least `length`."""
    return -(-length // ALIGNMENT) * ALIGNMENT


def encode_request(request_id: int, command: dict, buffers: Sequence[bytes] = ()) -> bytes:
    """A request frame holding one request with `command` and the `u8` buffers it refers to."""
    table = []
    offset = 0
    for data in buffers:
        table.append({"offset": offset, "length": len(data), "element_type": "u8"})
        offset = aligned(offset + len(data))
    header = json.dumps(
        {
            "protocol_version": PROTOCOL_VERSION,
            "messages": [{"kind": "request", "id": request_id, "command": command}],
            "buffers": table,
        },
        separators=(",", ":"),
        ensure_ascii=False,
        allow_nan=False,
    ).encode("utf-8")
    frame = bytearray(MAGIC)
    frame += len(header).to_bytes(4, "little")
    frame += header
    frame += bytes(aligned(len(header)) - len(header))
    for entry, data in zip(table, buffers, strict=True):
        start = PREFIX_LENGTH + aligned(len(header)) + entry["offset"]
        frame += bytes(start - len(frame))
        frame += data
    return bytes(frame)


def decode_response(frame: bytes) -> Frame:
    """The events, response and buffers of a response frame.

    Raises PartsmithError when the frame does not follow the layout, which only an engine bug
    can cause.
    """
    if frame[:4] != MAGIC or len(frame) < PREFIX_LENGTH:
        raise _malformed("it does not start with PSM1")
    header_length = int.from_bytes(frame[4:PREFIX_LENGTH], "little")
    header_end = PREFIX_LENGTH + header_length
    if len(frame) < header_end:
        raise _malformed("its header is cut off")
    try:
        header = json.loads(frame[PREFIX_LENGTH:header_end].decode("utf-8"))
    except ValueError as error:
        raise _malformed(f"its header is not JSON ({error})") from error
    if header.get("protocol_version") != PROTOCOL_VERSION:
        raise _malformed(f"it has protocol version {header.get('protocol_version')}")
    messages = header.get("messages") or []
    if not messages or messages[-1].get("kind") != "response":
        raise _malformed("it has no response message")
    buffer_start = PREFIX_LENGTH + aligned(header_length)
    buffers = []
    for entry in header.get("buffers", []):
        start = buffer_start + entry["offset"]
        end = start + entry["length"]
        if end > len(frame):
            raise _malformed("a buffer runs past its end")
        buffers.append(bytes(frame[start:end]))
    return Frame(events=messages[:-1], response=messages[-1], buffers=buffers)


def _malformed(reason: str) -> PartsmithError:
    """The error for a response frame that breaks the layout."""
    return PartsmithError(f"The engine returned a malformed frame: {reason}.")
