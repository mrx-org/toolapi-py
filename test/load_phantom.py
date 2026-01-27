# TODO: This should be moved into mr0 when ready
import json
import MRzeroCore as mr0
import numpy as np
import nibabel as nib
from pathlib import Path
from typing import Literal
import torch
from functools import lru_cache


class PhantomDict(dict[str, mr0.VoxelGridPhantom]):
    @classmethod
    def load(cls, path: str):
        return load_phantom(path)

    @property
    def tissues(self) -> list[str]:
        return list(self.keys())
    
    def interpolate(self, x: int, y: int, z: int):
        return PhantomDict({
            name: phantom.interpolate(x, y, z) for name, phantom in self.items()
        })
    
    def slices(self, slices: list[int]):
        return PhantomDict({
            name: phantom.slices(slices) for name, phantom in self.items()
        })
    
    def combine(self) -> mr0.VoxelGridPhantom:
        """Combine individual maps to mixed-tissue (no partial volume) phantom."""
        PD = sum(p.PD for p in self.values())
        PD[PD == 0] = 1
        segmentation = [p.PD / PD for p in self.values()]
        
        from copy import deepcopy
        phantoms = list(self.values())
        combined = deepcopy(phantoms[0])
        combined.PD = PD
        combined.T1 = sum(seg * p.T1 for seg, p in zip(segmentation, phantoms))
        combined.T2 = sum(seg * p.T2 for seg, p in zip(segmentation, phantoms))
        combined.T2dash = sum(seg * p.T2dash for seg, p in zip(segmentation, phantoms))
        combined.D = sum(seg * p.D for seg, p in zip(segmentation, phantoms))
        combined.B0 = sum(seg * p.B0 for seg, p in zip(segmentation, phantoms))
        combined.B1 = sum(seg[None, ...] * p.B1 for seg, p in zip(segmentation, phantoms))
        combined.coil_sens = sum(seg[None, ...] * p.coil_sens for seg, p in zip(segmentation, phantoms))

        return combined

    def build(self, PD_threshold: float = 1e-6,
              voxel_shape: Literal["sinc", "box", "point"] = "sinc"
              ) -> mr0.SimData:
        data_list = [
            self[tissue].build(PD_threshold, voxel_shape)
            for tissue in self.tissues
        ]
        return mr0.SimData(
            PD=torch.cat([obj.PD for obj in data_list]),
            T1=torch.cat([obj.T1 for obj in data_list]),
            T2=torch.cat([obj.T2 for obj in data_list]),
            T2dash=torch.cat([obj.T2dash for obj in data_list]),
            D=torch.cat([obj.D for obj in data_list]),
            B0=torch.cat([obj.B0 for obj in data_list]),
            B1=torch.cat([obj.B1 for obj in data_list], 1),
            coil_sens=torch.cat([obj.coil_sens for obj in data_list], 1),
            voxel_pos=torch.cat([obj.voxel_pos for obj in data_list], 0),
            size=data_list[0].size,
            nyquist=data_list[0].nyquist,
            dephasing_func=data_list[0].dephasing_func,
        )

        


DEFAULTS: dict[str, float | np.ndarray] = {
    "T1": float("inf"),
    "T2": float("inf"),
    "T2dash": float("inf"),
    "ADC": 0.0,
    "B0": 0.0,
    "B1": 1.0,
}


def load_phantom(path: str) -> PhantomDict:
    with open(path) as f:
        meta = json.load(f)
    assert meta["file_type"] == "voxel_tissue_v1"
    tissues = meta["tissues"].items()
    dir = Path(path).parent

    return PhantomDict({
        name: load_tissue(config, dir) for name, config in tissues
    })


def load_tissue(tissue_config, dir: Path) -> mr0.VoxelGridPhantom:
    # load PD from file, it *must* be there
    PD, PD_affine = load_nifti(dir, tissue_config["PD"])
    size = np.abs(PD.shape @ PD_affine[:3, :3]) / 1000  # mm to m

    # TODO: affine transformations are currently ignored.
    # Could give phantom that property and respect it when building voxel_pos?

    props = {
        "PD": PD,
        "size": size,
        "coil_sens": np.ones((1, *PD.shape)),
    }

    for map_name in DEFAULTS:
        # Read constant value for property
        try:
            value = float(tissue_config[map_name])
            props[map_name] = np.ones_like(PD) * value
        
        # Read from NIfTI + mapping func
        except TypeError:
            file = tissue_config[map_name]["file"]
            func = tissue_config[map_name]["func"]
            data, data_affine = load_nifti(dir, file)
            assert data.shape == PD.shape
            assert np.all(data_affine == PD_affine)

            print(f"Executing mapping function: '{func}'")
            # Apply the mapping func, giving access to some predefined vars.
            # TODO: Don't use eval, only allow simple arithmetic and these vars:
            props[map_name] = eval(func, {"__builtins__": None}, {
                "x": data,
                "x_min": data.min(),
                "x_max": data.max(),
                "x_mean": data.mean(),
                "x_std": data.std()
            })

        # Read from NIfTI
        except ValueError:
            file = tissue_config[map_name]
            data, data_affine = load_nifti(dir, file)
            assert data.shape == PD.shape
            assert np.all(data_affine == PD_affine)

            props[map_name] = data

        # Use default value
        except KeyError:
            props[map_name] = np.ones_like(PD) * DEFAULTS[map_name]

    # Fixup renamings
    props["D"] = props.pop("ADC")

    # Add coil dimension
    if props["B1"].ndim == 3:
        props["B1"] = props["B1"][None, ...]

    return mr0.VoxelGridPhantom(**props)


def load_nifti(dir: Path, file_str: str) -> tuple[np.ndarray, np.ndarray]:
    print(f"[NIfTI] {file_str}")
    res = file_str.split(":")
    
    # Folder has : of itws own, lets put the path back together
    if len(res) > 2:
        index = res[-1]
        file = ":".join(res[:-1])
        index = int(index)
    else:
        file, index = res
        file = dir / file
        index = int(index)

    img = _load_cached(file)
    if len(img.shape) == 4:
        data = img.get_fdata(dtype=np.float32)[:, :, :, index]
    else: # case for B1 written in breast phantom
        data = img.get_fdata(dtype=np.float32)[0, :, :, :, index]
    affine = img.get_sform()
    return data, affine

@lru_cache(16)
def _load_cached(file):
    print(f"[NIfTI LOAD] {file}")
    return nib.loadsave.load(file)