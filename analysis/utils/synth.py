from typing import Optional

import attr
import numpy as np


def get_amp(harmonic: int) -> float:
    if harmonic % 2 == 0:
        return -1.0
    else:
        return 1.0


@attr.dataclass(frozen=True)
class FreqComponent:
    freq: float
    amp: float
    divisor: float


TAU = 2.0 * np.pi


def create_saw(
    sample_rate: float,
    n: int,
    f0: float,
    harmonics: int = 48,
    cut_at: Optional[float] = None,
):
    """
    Args:
        n: Length of returned signal.
        f0: fundamental frequency
        harmonics: Number of sawtooth harmonics.
        cut_at: Frequency to cut off. Defaults to Nyquist.
    """
    components = [
        FreqComponent(freq=i * f0, amp=get_amp(i), divisor=float(i))
        for i in range(1, harmonics + 1)
    ]

    nyquist = sample_rate / 2.0
    cut_at = cut_at or nyquist

    dt = 1.0 / sample_rate
    time = 0.0

    out = np.zeros(n)
    for i in range(n):
        v = 0.0
        for freq_component in components:
            if freq_component.freq >= cut_at:
                continue
            v += freq_component.amp * (
                np.sin(TAU * freq_component.freq * time) / freq_component.divisor
            )
        out[i] = v
        time += dt
    return out
