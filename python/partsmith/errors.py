"""Exception and warning classes (python-api.md section 4)."""


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


class FeatureError(PartsmithError):
    """A feature (or sketch) did not build; the document no longer contains it."""

    def __init__(self, message: str, *, feature_type: str, code: str | None = None) -> None:
        super().__init__(message, code=code)
        self.feature_type = feature_type


class SelectionError(PartsmithError):
    """A selection is empty, has the wrong kind of elements, or has no part to select from."""


class ExpressionError(PartsmithError):
    """An expression does not parse or evaluate."""


class PartsmithWarning(UserWarning):
    """A feature built with warnings, or an export has a warning."""
