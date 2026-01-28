"""Pure Python wrapper classes for toolapi Value types.

These classes mirror the Rust Value enum variants. The Rust ``obj_to_value``
converter dispatches on the Python class name.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass
class TissueProperties:
    t1: float
    t2: float
    t2dash: float
    adc: float


@dataclass
class VoxelGridPhantom:
    voxel_shape_type: str
    voxel_shape_data: list[float]
    grid_spacing: list[float]
    grid_size: list[int]
    pd: list[float]
    t1: list[float]
    t2: list[float]
    t2dash: list[float]
    adc: list[float]
    b0: list[float]
    b1: list[list[float]]
    coil_sens: list[list[float]]


@dataclass
class MultiTissuePhantom:
    voxel_shape_type: str
    voxel_shape_data: list[float]
    grid_spacing: list[float]
    grid_size: list[int]
    b1: list[list[float]]
    coil_sens: list[list[float]]
    tissues: list[tuple[list[float], list[float], TissueProperties]]


class Event:
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


@dataclass
class EventSeq:
    events: list[Event]


@dataclass
class BlockSeq:
    blocks: list


@dataclass
class Block:
    min_duration: float = 0.0
    rf: object = None
    gx: object = None
    gy: object = None
    gz: object = None
    adc: object = None
