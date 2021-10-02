from typing import Optional, Tuple

import matplotlib.pyplot as plt
import numpy as np
import numpy.fft

# Useful constants
C0 = -57
C8 = 39


def freq_for(note: int) -> float:
    base_note = float(note - 69)
    return 440.0 * (2.0 ** (base_note / 12.0))


def fft_plot(
    signal: np.ndarray,
    sample_rate: float,
    log_freq: bool = False,
    ax: Optional[plt.Axes] = None,
):
    xf, y_db = fft(signal, sample_rate)

    if ax is None:
        _fig, ax = plt.subplots(figsize=(9, 4))
    ax.set_ylabel("Amplitude (dB)")
    if log_freq:
        ax.semilogx(xf, y_db)
    else:
        ax.plot(xf, y_db)
    # plt.show()
    return ax


def fft(signal: np.ndarray, sample_rate: float) -> Tuple[np.ndarray, np.ndarray]:
    t = 1.0 / sample_rate
    n = len(signal)
    yf = numpy.fft.rfft(signal)
    xf = numpy.fft.rfftfreq(n, t)[: len(yf)]
    y_db = 20.0 * np.log10(yf / np.max(np.abs(yf)))

    return xf, y_db


def smooth_edges(signal: np.ndarray, amt: int) -> np.ndarray:
    if amt == 0:
        return signal
    # Smooth out the edges so that we avoid high frequency artifacts.
    signal = np.array(signal)
    if signal[0] != 0.0:
        target = 1.0  # np.max(np.abs(signal[amt - 3 : amt + 3]))
        mask = np.linspace(0.0, target, amt)
        signal[0 : len(mask)] = signal[0 : len(mask)] * mask
    if signal[-1] != 0.0:
        target = 1.0  # np.max(np.abs(signal[-amt - 3 : -amt + 3]))
        mask = np.linspace(target, 0.0, amt)
        signal[-len(mask) :] *= mask
    return signal
