"""Pure Python wrapper classes for toolapi Value types.

These classes mirror the Rust Value enum variants. Each instance carries a
``_type`` string that the Rust ``obj_to_value`` converter uses for dispatch.
"""

from __future__ import annotations


class TissueProperties:
    _type = "TissueProperties"

    def __init__(self, t1: float, t2: float, t2dash: float, d: float):
        self.t1 = t1
        self.t2 = t2
        self.t2dash = t2dash
        self.d = d  # maps to Rust field `adc`

    def __repr__(self):
        return f"TissueProperties(t1={self.t1}, t2={self.t2}, t2dash={self.t2dash}, d={self.d})"


class VoxelGridPhantom:
    _type = "VoxelGridPhantom"

    def __init__(
        self,
        voxel_shape_type: str,
        voxel_shape_data: list[float],
        grid_spacing: list[float],
        grid_size: list[int],
        pd: list[float],
        t1: list[float],
        t2: list[float],
        t2dash: list[float],
        adc: list[float],
        b0: list[float],
        b1: list[list[float]],
        coil_sens: list[list[float]],
    ):
        self.voxel_shape_type = voxel_shape_type
        self.voxel_shape_data = voxel_shape_data
        self.grid_spacing = grid_spacing
        self.grid_size = grid_size
        self.pd = pd
        self.t1 = t1
        self.t2 = t2
        self.t2dash = t2dash
        self.adc = adc
        self.b0 = b0
        self.b1 = b1
        self.coil_sens = coil_sens

    def __repr__(self):
        return f"VoxelGridPhantom(grid_size={self.grid_size})"


class MultiTissuePhantom:
    _type = "MultiTissuePhantom"

    def __init__(
        self,
        voxel_shape_type: str,
        voxel_shape_data: list[float],
        grid_spacing: list[float],
        grid_size: list[int],
        b1: list[list[float]],
        coil_sens: list[list[float]],
        tissues: list[tuple[list[float], list[float], TissueProperties]],
    ):
        self.voxel_shape_type = voxel_shape_type
        self.voxel_shape_data = voxel_shape_data
        self.grid_spacing = grid_spacing
        self.grid_size = grid_size
        self.b1 = b1
        self.coil_sens = coil_sens
        self.tissues = tissues

    def __repr__(self):
        return f"MultiTissuePhantom(grid_size={self.grid_size}, tissues={len(self.tissues)})"


class Event:
    _type = "Event"

    def __init__(self, variant: str, **kwargs):
        self.variant = variant
        self.fields = kwargs

    @staticmethod
    def Pulse(angle: float, phase: float) -> Event:
        return Event("Pulse", angle=angle, phase=phase)

    @staticmethod
    def Fid(kt: list[float]) -> Event:
        return Event("Fid", kt=kt)

    @staticmethod
    def Adc(phase: float) -> Event:
        return Event("Adc", phase=phase)

    def __repr__(self):
        return f"Event.{self.variant}({self.fields})"


class EventSeq:
    _type = "EventSeq"

    def __init__(self, events: list[Event]):
        self.events = events

    def __repr__(self):
        return f"EventSeq({len(self.events)} events)"


class BlockSeq:
    _type = "BlockSeq"

    def __init__(self, blocks: list):
        self.blocks = blocks

    def __repr__(self):
        return f"BlockSeq({len(self.blocks)} blocks)"


class Block:
    _type = "Block"

    def __init__(self, min_duration: float = 0.0, rf=None, gx=None, gy=None, gz=None, adc=None):
        self.min_duration = min_duration
        self.rf = rf
        self.gx = gx
        self.gy = gy
        self.gz = gz
        self.adc = adc

    def __repr__(self):
        return f"Block(min_duration={self.min_duration})"
