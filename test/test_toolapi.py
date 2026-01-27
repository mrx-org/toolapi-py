# %% Imports
import MRzeroCore as mr0
import torch
import matplotlib.pyplot as plt
from time import time

import toolapi
import util
from load_phantom import PhantomDict

# %% Prepare sequence and phantom
seq = mr0.Sequence.import_file("assets/gre.seq")  # gre.seq tse.seq
seq = util.to_instant_events(seq)

phantom = PhantomDict.load("assets/brainweb-subj05/brainweb-subj05-3T.json")
phantom = phantom.interpolate(64, 64, 64).slices([30])
for tissue in phantom.values():
    tissue.D[:] = 0
    # tissue.T2[:] = 1e9
    # tissue.T2dash[:] = 1e9
    # tissue.B0[:] = 0
    tissue.B1[:] = 1
phantom = util.phantom_dict_to_toolapi(phantom)


# %% Run the simulation tool
def sim_spdg(sequence, phantom):
    def on_message(msg):
        print(f"[MESSAGE]: {msg}")
        return True

    return toolapi.call(
        "wss://tool-spdg-flyio.fly.dev/tool",
        on_message,
        sequence=sequence,
        phantom=phantom,
    )

# %% Run the simulation!
start = time()
signal = torch.tensor(sim_spdg(seq, phantom))[0, :]
end = time()

print(f"Tool took {end - start:.3} s")

plt.figure()
plt.plot(signal.abs())
plt.grid()
plt.show()
