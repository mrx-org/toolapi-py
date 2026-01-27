import MRzeroCore as mr0
import torch
import toolapi
from load_phantom import PhantomDict

def mr0_phantom_to_toolapi(phantom: mr0.VoxelGridPhantom):
    assert isinstance(phantom, mr0.VoxelGridPhantom)
    voxel_size = (phantom.size / torch.as_tensor(phantom.PD.shape)).tolist()
    return toolapi.value.VoxelGridPhantom(
        "AASinc",
        voxel_size,
        voxel_size,
        list(phantom.PD.shape),
        phantom.PD.flatten().tolist(),
        phantom.T1.flatten().tolist(),
        phantom.T2.flatten().tolist(),
        phantom.T2dash.flatten().tolist(),
        phantom.D.flatten().tolist(),
        phantom.B0.flatten().tolist(),
        phantom.B1.reshape(phantom.B1.shape[0], -1).tolist(),
        phantom.coil_sens.reshape(phantom.coil_sens.shape[0], -1).tolist()
    )

def phantom_dict_to_toolapi(phantom: PhantomDict) -> toolapi.value.MultiTissuePhantom:
    tissues = list(phantom.values())
    voxel_size = (tissues[0].size / torch.as_tensor(tissues[0].PD.shape)).tolist()
    
    return toolapi.value.MultiTissuePhantom(
        "AASinc",  # voxel shape type
        voxel_size,  # voxel shape data
        voxel_size,  # grid spacing
        list(tissues[0].PD.shape),  # grid size
        # We assume that all tissues have the same B1 and coil_sens
        tissues[0].B1.reshape(tissues[0].B1.shape[0], -1).tolist(),
        tissues[0].coil_sens.reshape(tissues[0].coil_sens.shape[0], -1).tolist(),
        [
            (
                tissue.PD.flatten().tolist(),
                tissue.B0.flatten().tolist(),
                toolapi.value.TissueProperties(
                    float(tissue.T1[tissue.PD > 0].mean()),
                    float(tissue.T2[tissue.PD > 0].mean()),
                    float(tissue.T2dash[tissue.PD > 0].mean()),
                    float(tissue.D[tissue.PD > 0].mean()),
                )
            )
            for tissue in tissues
        ]
    )


def to_instant_events(seq: mr0.Sequence) -> toolapi.value.EventSeq:
    ie_seq = []

    Event = toolapi.value.BlockSeq
    for rep in seq:
        ie_seq.append(Event.Pulse(rep.pulse.angle, rep.pulse.phase))
        for ev in range(rep.event_count):
            ie_seq.append(Event.Fid([
                rep.gradm[ev, 0],
                rep.gradm[ev, 1],
                rep.gradm[ev, 2],
                rep.event_time[ev]
            ]))
            if rep.adc_usage[ev] != 0:
                ie_seq.append(Event.Adc(torch.pi / 2 - rep.adc_phase[ev]))

    ie_seq = toolapi.value.EventSeq(ie_seq)
    return ie_seq