# %% Imports
import MRzeroCore as mr0
import torch
import matplotlib.pyplot as plt
from time import time

import toolapi
import util
from load_phantom import PhantomDict

# %% Prepare sequence and phantom
seq = mr0.Sequence.import_file("test/assets/gre.seq")  # gre.seq tse.seq
seq = util.to_instant_events(seq)

phantom = PhantomDict.load("test/assets/brainweb-subj05/brainweb-subj05-3T.json")
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
signal = torch.tensor(sim_spdg(seq, phantom)["signal"])[0, :]
end = time()

print(f"Tool took {end - start:.3} s")

kspace = signal.reshape(256, 256)
reco = torch.fft.fftshift(torch.fft.fft2(torch.fft.fftshift(kspace)))

plt.figure()
plt.subplot(211)
plt.plot(signal.abs())
plt.grid()
plt.subplot(223)
plt.imshow(reco.abs(), origin="lower", vmin=0)
plt.axis("off")
plt.subplot(224)
plt.imshow(reco.angle(), origin="lower", vmin=-torch.pi, vmax=torch.pi, cmap="twilight")
plt.axis("off")
plt.show()
