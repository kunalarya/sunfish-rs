import argparse
from typing import Optional

import matplotlib
import matplotlib.pyplot as plt
import numpy as np
import seaborn as sns

import pysunfish
import utils.common
import utils.interface as interface
import utils.synth

matplotlib.use("Qt5Agg")

sns.set_style("whitegrid")

# Sample rate
SAMPLE_RATE = 44100
SMOOTH_SAMPLES = 500
TAU = 2.0 * np.pi


# Default arguments
DEFAULT_CHUNK_SIZE = 1024
DEFAULT_SHAPE = "Sine"
DEFAULT_NOTE_START = 30
DEFAULT_NOTE_END = 155
DEFAULT_LENGTH = 0.4
DEFAULT_COLORMAP = "viridis"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument(
        "--spectra", action="store_true", help="Sweep notes, plot spectrum."
    )
    parser.add_argument(
        "--single", action="store_true", help="Plot a single notes' worth of waveforms."
    )
    parser.add_argument(
        "--note-start",
        "--note",
        action="store",
        type=int,
        help="Note for either single or starting value (MIDI code) (default: {DEFAULT_NOTE_START}).",
        default=DEFAULT_NOTE_START,
    )
    parser.add_argument(
        "--note-end",
        action="store",
        type=int,
        help=f"Ending note for sweep (MIDI code) (default: {DEFAULT_NOTE_END}).",
        default=DEFAULT_NOTE_END,
    )
    parser.add_argument(
        "--shape",
        action="store",
        help=f"Waveform type (default: {DEFAULT_SHAPE}).",
        default=DEFAULT_SHAPE,
    )
    parser.add_argument(
        "--length",
        type=float,
        help=f"Length, in seconds, for waveforms (default: {DEFAULT_LENGTH}).",
        default=DEFAULT_LENGTH,
    )
    parser.add_argument(
        "--cut-start",
        type=int,
        help=f"Optional signal cut, in samples.",
    )
    parser.add_argument(
        "--cut-end",
        type=int,
        help=f"Optional signal cut end, in samples.",
    )
    parser.add_argument(
        "--smooth",
        type=int,
        help=f"Number of samples to ramp up/down (default: {SMOOTH_SAMPLES}).",
        default=SMOOTH_SAMPLES,
    )
    parser.add_argument(
        "--colormap",
        type=str,
        help=f"Name of the colormap for spectral plots (default: {DEFAULT_COLORMAP}).",
        default=DEFAULT_COLORMAP,
    )
    parser.add_argument(
        "--chunk-size",
        type=int,
        action="store",
        help=f"Chunk size for rendering (default: {DEFAULT_CHUNK_SIZE})",
        default=DEFAULT_CHUNK_SIZE,
    )
    args = parser.parse_args()
    if args.spectra:
        heatmap(
            shape=args.shape,
            length_sec=args.length,
            note_start=args.note_start,
            note_end=args.note_end,
            smooth_samples=args.smooth,
            color_map=args.colormap,
        )
    elif args.single:
        plot_waves(
            time_sec=args.length,
            note=args.note_start,
            shape=args.shape,
            chunk_size=args.chunk_size,
            cut_start=args.cut_start,
            cut_end=args.cut_end,
            smooth_samples=args.smooth,
        )
    else:
        print("No action specified")


def plot_single_cycle(note: int):
    freq = utils.common.freq_for(note)
    single_cycle_s = 1.0 / freq
    plot_waves(
        time_sec=single_cycle_s,
        shape="sine",
        smooth_samples=0,
        note=note,
    )


def plot_waves(
    time_sec: float,
    note: int,
    shape: str,
    smooth_samples: int,
    chunk_size: int,
    cut_start: Optional[int] = None,
    cut_end: Optional[int] = None,
):

    buf_len = int(time_sec * SAMPLE_RATE)

    # Create perfect Sine wave.
    freq = utils.common.freq_for(note)

    # Create the time axis
    ts = np.linspace(0.0, time_sec, buf_len)
    ys = np.sin(TAU * freq * ts)

    _fig, axes = plt.subplots(nrows=2, ncols=1)

    synth = pysunfish.CoreWrapper(SAMPLE_RATE)
    initialize_synth(synth, shape)

    synth.note_on(note)
    signal_left, _signal_right = synth.render(chunk_size, buf_len, shape)

    start = 0
    end = buf_len
    if cut_start is not None:
        start = cut_start
    if cut_end is not None:
        end = cut_end

    # How many points to plot:
    plot_len = end - start

    signal = signal_left[start:end]

    # Create the rendered sample.
    signal = utils.common.smooth_edges(signal, smooth_samples)

    axes[0].plot(signal, alpha=0.6, color="r")

    amp = 1.0
    ys_plot = amp * utils.common.smooth_edges(ys[:plot_len], smooth_samples)
    axes[0].plot(ys_plot, alpha=0.6, color="b")

    # FFT below.
    xf, y_db = utils.common.fft(signal, SAMPLE_RATE)
    axes[1].plot(xf, y_db, color="b")

    plt.tight_layout()
    plt.show()


def shape_float(shape: str) -> float:
    shape = shape.lower().strip()
    if shape == "sine":
        return 0.0
    elif shape == "softsaw":
        return 0.4
    elif shape == "hardsaw":
        return 0.7
    else:
        raise ValueError(f"Unsupported shape: {shape}")


def heatmap(
    shape: str,
    length_sec: float,
    note_start: int,
    note_end: int,
    smooth_samples: int,
    color_map: str,
) -> None:
    samples = int(length_sec * SAMPLE_RATE)
    chunk_size = 1024

    # These should be the same as interpolator.rs, same names:
    SOFT_SAW_HARMONICS = 8
    HARD_SAW_HARMONICS = 64

    shape = shape.lower().strip()
    if shape == "sine":
        harmonics = 1
    elif shape == "softsaw":
        harmonics = SOFT_SAW_HARMONICS
    elif shape == "hardsaw":
        harmonics = HARD_SAW_HARMONICS
    else:
        raise ValueError(f"Unsupported shape: {shape}")

    spectra = []
    spectra_ideal = []

    # for note in tqdm(range(note_start, note_end)):
    for note in range(note_start, note_end):
        # Create perfect Sine wave.
        freq = utils.common.freq_for(note)

        # Create the time axis
        ts = np.linspace(0.0, length_sec, samples)
        signal = utils.synth.create_saw(
            SAMPLE_RATE, samples, f0=freq, harmonics=harmonics
        )
        signal_smooth = utils.common.smooth_edges(signal, amt=smooth_samples)
        xf, y_db = utils.common.fft(signal_smooth, SAMPLE_RATE)
        spectra_ideal.append(y_db)

        print(f"{samples=} {note=}")

        synth = pysunfish.CoreWrapper(SAMPLE_RATE)
        initialize_synth(synth, shape)

        synth.note_on(note)
        l, _r = synth.render(chunk_size, samples, shape)
        synth.note_off(note)

        signal_smooth = utils.common.smooth_edges(signal, amt=smooth_samples)
        xf, y_db = utils.common.fft(signal_smooth, SAMPLE_RATE)
        spectra.append(y_db)

    spectra_ideal = np.array(spectra_ideal).T
    spectra = np.array(spectra).T

    fig, ax = plt.subplots(nrows=2, ncols=1)
    cmap = sns.color_palette(color_map, as_cmap=True)

    sns.heatmap(-np.abs(spectra_ideal), ax=ax[0], cmap=cmap, vmin=-130)
    sns.heatmap(-np.abs(spectra), ax=ax[1], cmap=cmap, vmin=-130)
    plt.tight_layout()
    plt.show()


def initialize_synth(synth, shape: str) -> None:
    synth.update_param(interface.eparam_path("Osc1", "Enable"), 1.0)
    synth.update_param(interface.eparam_path("Osc1", "Shape"), shape_float(shape))
    synth.update_param(interface.eparam_path("Osc2", "Enable"), 0.0)

    synth.update_param(interface.eparam_path("Filt1", "Enable"), 0.0)
    synth.update_param(interface.eparam_path("Filt2", "Enable"), 0.0)

    synth.update_param(interface.eparam_path("AmpEnv", "Attack"), 0.1)
    synth.update_param(interface.eparam_path("AmpEnv", "Sustain"), 1.0)
    synth.update_param(interface.eparam_path("AmpEnv", "Decay"), 1.0)
    synth.update_param(interface.eparam_path("AmpEnv", "Release"), 1.0)


if __name__ == "__main__":
    main()
