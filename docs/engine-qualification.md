# Engine qualification — November 15, 2026

## Implemented

The initial Windows 11 x64 command-line prototype lists monitors, records a selected monitor to silent H.264 MP4 at 30 fps, scales proportionally within 1920 x 1080, and exposes idle/capture/recording memory checks. It has a single latest-frame slot and synchronous Media Foundation sample submission. A fixed output timeline repeats static content and errors when encoding falls substantially behind.

Each recording is a short-lived worker. The controller assigns a kill-on-close Windows job before sending a pipe start signal. The worker exits after finalization, releasing retained media allocations. Temporary output is renamed after successful finalization. Failure paths attempt to preserve a finalized partial recording.

## Measurements and corrections

Development hardware: Intel Arc 140T GPU, driver 32.0.101.8801, 3000 x 1876 monitor.

- Initial capture-only runs used approximately 31 MiB private memory, returning to 17.5 MiB.
- Initial native-resolution recording reached approximately 1591 MiB private memory in ten seconds.
- Limiting encoded dimensions, requesting hardware transforms and low latency reduced a 30-second test to approximately 173–202 MiB private memory. Output was 1726 x 1080, 900 frames.
- Repeated same-process encoding retained 20.7, 97.2 and 100.8 MiB after successive sessions. This motivated process isolation.
- Final worker-based two-cycle measurement used approximately 3.28 MiB private memory between sessions and 181–227 MiB across controller, console helper and worker while recording.
- Final two-second output contained 60 H.264 frames. Independent verification also inspected a three-second H.264 file with 90 frames at 30 fps, 1726 x 1080.
- Initial output orientation was corrected and test decode succeeded.
- The measurement script was corrected for Windows PowerShell 5.1 compatibility and live-process sampling.

These are short development-machine measurements. Private bytes and working set exclude separately allocated GPU memory. Hardware encoding was requested but the selected encoder was not identified. A native GUI has not been implemented or measured.

## Validation and remaining work

Formatting, release compilation, Clippy with warnings denied, and Cargo test succeed. Cargo currently discovers zero automated tests. Real short recordings and process cleanup were exercised. The independent verifier reviewed the code and ran additional recording and memory checks.

Long-session stability, peak GPU memory, recording CPU budget, internal Media Foundation queue limits, hardware fallback behavior, disk-full handling and the full multi-monitor matrix still require qualification. The current CPU readback and nearest-neighbor resize are prototype tradeoffs. Fixed 1920 x 1080 padded canvas is future preset work; this prototype preserves aspect ratio using dimensions within that limit.

Window/region selection, microphone, click effects, output presets and the native control window remain pending as specified in implementation-plan.md.

Generated recordings and screenshots are disposable test artifacts. Delete them immediately after inspecting codec, duration and playback. Retain numeric results only; do not commit capture content.

Final independent pass measured 195–225 MiB private memory during two recordings and 3.29 MiB between cycles (approximately 17.9 MiB working set). Both two-second files independently probed as H.264, 1726 x 1080, 30 fps, 60 frames. No worker remained afterward.
