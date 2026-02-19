"""Pure Python wrapper classes for toolapi Value types.

These classes mirror the Rust Value enum variants. The Rust ``obj_to_value``
converter dispatches on the Python class name.

Primitive types (None, bool, int, float, str, complex) map directly to
Python builtins and do not need wrapper classes.

Dynamic containers (Dict, List) map to Python dict/list of Value objects.

TypedList and TypedDict are represented as plain Python lists/dicts whose
element types are inferred on the Rust side.
"""

from __future__ import annotations

from dataclasses import dataclass, field


# -- Atomic vector types -----------------------------------------------------


@dataclass
class Vec3:
    """3-element float vector.  Rust: ``Vec3(pub [f64; 3])``."""

    data: list[float]  # must have exactly 3 elements

    def __post_init__(self):
        if len(self.data) != 3:
            raise ValueError(f"Vec3 requires exactly 3 elements, got {len(self.data)}")


@dataclass
class Vec4:
    """4-element float vector.  Rust: ``Vec4(pub [f64; 4])``."""

    data: list[float]  # must have exactly 4 elements

    def __post_init__(self):
        if len(self.data) != 4:
            raise ValueError(f"Vec4 requires exactly 4 elements, got {len(self.data)}")


# -- Structured types --------------------------------------------------------


@dataclass
class Volume:
    """3D voxel volume with affine transform.

    Rust::

        pub struct Volume {
            pub shape: [u64; 3],
            pub affine: [[f64; 4]; 3],
            pub data: TypedList,
        }

    On the Python side, ``data`` is a plain list whose element type is
    inferred by the Rust converter (e.g. list[float], list[complex], ...).
    ``affine`` is a list of 3 lists of 4 floats each.
    """

    shape: list[int]  # [u64; 3]
    affine: list[list[float]]  # [[f64; 4]; 3]
    data: list  # TypedList – element type inferred


@dataclass
class PhantomTissue:
    """Single tissue with density/db0 volumes and relaxation parameters.

    Rust::

        pub struct PhantomTissue {
            pub density: Volume,
            pub db0: Volume,
            pub t1: f64,
            pub t2: f64,
            pub t2dash: f64,
            pub adc: f64,
        }
    """

    density: Volume
    db0: Volume
    t1: float
    t2: float
    t2dash: float
    adc: float


@dataclass
class SegmentedPhantom:
    """Multi-tissue segmented phantom.

    Rust::

        pub struct SegmentedPhantom {
            pub tissues: HashMap<String, PhantomTissue>,
            pub b1_tx: Vec<Volume>,
            pub b1_rx: Vec<Volume>,
        }
    """

    tissues: dict[str, PhantomTissue]
    b1_tx: list[Volume] = field(default_factory=list)
    b1_rx: list[Volume] = field(default_factory=list)


# -- Instant sequence event (enum) ------------------------------------------


class InstantSeqEvent:
    """Tagged-union style class mirroring the Rust InstantSeqEvent enum.

    Rust::

        pub enum InstantSeqEvent {
            Pulse { angle: f64, phase: f64 },
            Fid { kt: Vec4 },
            Adc { phase: f64 },
        }

    Construct via the static factory methods ``Pulse``, ``Fid``, ``Adc``.
    """

    def __init__(self, variant: str, **kwargs):
        self.variant = variant
        self.fields = kwargs

    @staticmethod
    def Pulse(angle: float, phase: float) -> InstantSeqEvent:
        return InstantSeqEvent("Pulse", angle=angle, phase=phase)

    @staticmethod
    def Fid(kt: Vec4 | list[float]) -> InstantSeqEvent:
        if isinstance(kt, list):
            kt = Vec4(kt)
        return InstantSeqEvent("Fid", kt=kt)

    @staticmethod
    def Adc(phase: float) -> InstantSeqEvent:
        return InstantSeqEvent("Adc", phase=phase)

    def __repr__(self):
        return f"InstantSeqEvent.{self.variant}({self.fields})"
