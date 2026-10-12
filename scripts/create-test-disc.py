#!/usr/bin/env python3
"""
OxiDisc - Test Disc Image Generator (CUE/BIN & Synthetic Audio)
Creates a virtual Red Book Audio CD image (test_disc.cue + test_disc.bin)
with synthetic 16-bit 44.1kHz stereo audio tracks matching the official MusicBrainz test TOC.
"""

import math
import struct
import sys
from pathlib import Path

SAMPLE_RATE = 44100
CHANNELS = 2
BYTES_PER_SAMPLE = 2
BYTES_PER_FRAME = CHANNELS * BYTES_PER_SAMPLE  # 4 bytes
FRAMES_PER_SECTOR = 588  # 44100 / 75 fps
BYTES_PER_SECTOR = FRAMES_PER_SECTOR * BYTES_PER_FRAME  # 2352 bytes

# 6 Tracks matching the official MusicBrainz test vector TOC:
# Offsets (sectors): 150, 15363, 32314, 46592, 63414, 80489, Leadout: 95462
TRACKS = [
    {"num": 1, "name": "Overture in C Major", "freq": 440.0, "sectors": 15213},
    {"num": 2, "name": "Harmonic Pulse", "freq": 554.37, "sectors": 16951},
    {"num": 3, "name": "Acoustic Reflection", "freq": 659.25, "sectors": 14278},
    {"num": 4, "name": "Binary Rhythm", "freq": 880.0, "sectors": 16822},
    {"num": 5, "name": "Spectral Waves", "freq": 987.77, "sectors": 17075},
    {"num": 6, "name": "Finale & Lead-Out", "freq": 523.25, "sectors": 14973},
]


def generate_cdda_sectors(freq: float, num_sectors: int) -> bytes:
    """Generates synthetic stereo 16-bit PCM CDDA sectors for a given frequency."""
    total_frames = num_sectors * FRAMES_PER_SECTOR
    data = bytearray(num_sectors * BYTES_PER_SECTOR)
    idx = 0

    two_pi_f = 2.0 * math.pi * freq
    two_pi_f_r = 2.0 * math.pi * (freq * 1.002)

    for i in range(total_frames):
        t = i / SAMPLE_RATE
        # Left channel
        s_left = math.sin(two_pi_f * t) * 0.6 + math.sin(two_pi_f * 2.0 * t) * 0.15
        # Right channel (subtle chorus detune)
        s_right = math.sin(two_pi_f_r * t) * 0.6 + math.sin(two_pi_f_r * 2.0 * t) * 0.15

        i_left = int(max(-32767, min(32767, s_left * 24000.0)))
        i_right = int(max(-32767, min(32767, s_right * 24000.0)))

        struct.pack_into("<hh", data, idx, i_left, i_right)
        idx += 4

    return bytes(data)


def create_test_disc(output_dir: Path, fast: bool = False):
    output_dir.mkdir(parents=True, exist_ok=True)
    bin_path = output_dir / "test_disc.bin"
    cue_path = output_dir / "test_disc.cue"

    print(f"Generating synthetic Audio CD image in: {output_dir}")
    print(f"Mode: {'Fast (short duration for testing)' if fast else 'Full TOC (matches official DiscID)'}")

    # Scale sectors if fast mode is requested
    sector_scale = 0.02 if fast else 1.0

    cue_lines = [
        f'FILE "{bin_path.name}" BINARY',
    ]

    current_sector = 0
    with open(bin_path, "wb") as bin_file:
        for t in TRACKS:
            track_sectors = max(150, int(t["sectors"] * sector_scale))
            print(f"  Generating Track {t['num']:02d}: '{t['name']}' ({t['freq']} Hz, {track_sectors} sectors)...")

            pcm_bytes = generate_cdda_sectors(t["freq"], track_sectors)
            bin_file.write(pcm_bytes)

            # CUE index time: MM:SS:FF
            m = current_sector // (60 * 75)
            s = (current_sector // 75) % 60
            f = current_sector % 75

            cue_lines.append(f'  TRACK {t["num"]:02d} AUDIO')
            cue_lines.append(f'    TITLE "{t["name"]}"')
            cue_lines.append(f'    PERFORMER "OxiDisc Audio Lab"')
            cue_lines.append(f'    INDEX 01 {m:02d}:{s:02d}:{f:02d}')

            current_sector += track_sectors

    cue_content = "\n".join(cue_lines) + "\n"
    with open(cue_path, "w", encoding="utf-8") as cue_file:
        cue_file.write(cue_content)

    print("\nSuccessfully generated Audio CD test disc:")
    print(f"  CUE Sheet: {cue_path}")
    print(f"  BIN Image: {bin_path} ({bin_path.stat().st_size / (1024*1024):.2f} MB)")
    print("\nUsage options:")
    print("  1. Hardware-Free Mock Mode directly in OxiDisc:")
    print("     oxidisc --mock-cd")
    print("  2. Mount using virtual CD software (WinCDEmu / Virtual CloneDrive / cdemu):")
    print(f"     Mount '{cue_path}' as a virtual drive letter and point OxiDisc to it.")


if __name__ == "__main__":
    fast_mode = "--fast" in sys.argv or "-f" in sys.argv
    out_dir = Path(__file__).parent.parent / "test_data"
    create_test_disc(out_dir, fast=fast_mode)
