"""Exception and warning classes (python-api.md section 4)."""

from __future__ import annotations

from collections.abc import Sequence
from typing import TYPE_CHECKING

if TYPE_CHECKING:
    from .selectors import Ref


class PartsmithError(Exception):
    """Base class of every Partsmith error.

    `code` is the protocol error code of an engine error response, or None for errors raised in
    Python.
    """

    def __init__(self, message: str, *, code: str | None = None) -> None:
        super().__init__(message)
        self.code = code


class EngineError(PartsmithError):
    """The engine answered a request with an error response."""


class _FeatureMessage:
    """What a feature message says beyond its text (python-api.md section 4), for the error and the
    warning that carry one.

    `feature` is the feature's id; `field` the JSON Pointer of the field, relative to the feature
    record; `suggestion` a replacement expression for the field; `actions` the labels of the
    message's one-click fixes, in order; `candidates` the possible replacements of a lost
    reference, each a `Ref` with `description` and `relation`; `cause` the cause line, set only
    when an edit of this session started the problem; `detail` the technical text for reports.
    """

    feature: str | None
    field: str | None
    suggestion: str | None
    actions: list[str]
    candidates: list[Ref]
    cause: str | None
    detail: str | None

    def _set_message(
        self,
        *,
        feature: str | None,
        field: str | None,
        suggestion: str | None,
        actions: Sequence[str],
        candidates: Sequence[Ref],
        cause: str | None,
        detail: str | None,
    ) -> None:
        """Sets the attributes of the message."""
        self.feature = feature
        self.field = field
        self.suggestion = suggestion
        self.actions = list(actions)
        self.candidates = list(candidates)
        self.cause = cause
        self.detail = detail


class FeatureError(PartsmithError, _FeatureMessage):
    """A feature (or sketch) did not build; the document no longer contains it.

    `feature_type` is the feature's type, and `code` the code of its error message (never None);
    the other attributes are those of the message (`_FeatureMessage`).
    """

    code: str

    def __init__(
        self,
        message: str,
        *,
        feature_type: str,
        feature: str,
        code: str,
        field: str | None = None,
        suggestion: str | None = None,
        actions: Sequence[str] = (),
        candidates: Sequence[Ref] = (),
        cause: str | None = None,
        detail: str | None = None,
    ) -> None:
        super().__init__(message, code=code)
        self.feature_type = feature_type
        self._set_message(
            feature=feature,
            field=field,
            suggestion=suggestion,
            actions=actions,
            candidates=candidates,
            cause=cause,
            detail=detail,
        )


class SelectionError(PartsmithError):
    """A selection is empty, has the wrong kind of elements, or has no part to select from."""


class ExpressionError(PartsmithError):
    """An expression does not parse or evaluate."""


class PartsmithWarning(UserWarning, _FeatureMessage):
    """A feature built with warnings, an edit started a problem, an export has a warning, or a
    selection was indexed by position.

    `code` is the diagnostic's code, or `selection_index` for indexing; a feature message also has
    the attributes of `_FeatureMessage`, which are None or empty for any other warning.
    """

    def __init__(
        self,
        message: str,
        *,
        code: str | None = None,
        feature: str | None = None,
        field: str | None = None,
        suggestion: str | None = None,
        actions: Sequence[str] = (),
        candidates: Sequence[Ref] = (),
        cause: str | None = None,
        detail: str | None = None,
    ) -> None:
        super().__init__(message)
        self.code = code
        self._set_message(
            feature=feature,
            field=field,
            suggestion=suggestion,
            actions=actions,
            candidates=candidates,
            cause=cause,
            detail=detail,
        )
