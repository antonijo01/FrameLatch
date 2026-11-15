# FrameLatch implementation plan

The detailed implementation units and everyday workflow are in [everyday-use-plan.md](everyday-use-plan.md). Current feature and resource observations are in [desktop-validation.md](desktop-validation.md). [engine-qualification.md](engine-qualification.md) retains the historical first-engine measurements. The checklists below describe the intended design; they do not claim all hardware qualification has passed.

Approved scope from the November 15, 2026 planning discussion.

FrameLatch is a minimal native Rust Windows screen recorder with low idle resource use and local compressed output. Initial platform is Windows 11 x64. The name is a working name whose availability has not been checked. Resource budgets below are targets requiring measurements.

## Complete first release

One native control window offers:
- One full monitor, one application window, or a drawn rectangle within one monitor.
- Microphone enabled/disabled and input-device selection before recording.
- Normal Windows cursor, with click highlight Off, Red ring (default), or Red ripple.
- Landscape, Vertical, and Small file output presets.
- Remembered output folder and settings, validated against currently connected devices.
- Record, elapsed time, Stop, and Open saved file.

Record one source per session. Closing the application exits it fully.

## 1. Qualify the engine

Begin with a small executable that lists/selects a monitor, records, and saves a playable H.264 MP4. A command-line qualification tool precedes the native control window.

Use Windows Graphics Capture. Qualify windows-capture before adopting its encoder. Its inspected 2.0.1 encoder uses unbounded channels and defaults to HEVC. Prefer a bounded integration or Windows Media Foundation via Rust Windows bindings when required.

- Keep a fixed small number of surfaces in flight.
- Replace stale pending frames if encoding falls behind.
- Preserve elapsed time when dropping frames.
- Produce a 30 fps timeline and repeat the latest image as needed on static content.
- Make stop independent of capture callbacks.
- Select H.264 explicitly and test encoder availability.
- Prefer hardware encoding. Explain software fallback and CPU cost before starting.

Acceptance: static content, scrolling and moving video produce correctly timed files. Queue sizes remain bounded through ten-minute qualification recordings.

## 2. Resource lifecycle

Use event-driven native Win32 UI. Create capture, graphics, microphone and encoder resources only when needed. Release selection previews after use and all recording resources after saving. Keep settings in a small local file.

Initial budgets:
- Open idle process private memory below 30 MiB.
- Idle CPU below 0.1% of total machine capacity averaged over 60 seconds.
- No ongoing idle capture or preview rendering.
- After recording, private memory within 10 MiB of the warmed idle baseline.
- No remaining process after closing.

Measure private bytes, working set and dedicated/shared GPU memory separately. Establish recording budgets empirically and verify stability over 30 minutes. A CLI idle baseline does not establish future GUI resource usage.

## 3. Source selection and framing

Confirm the chosen monitor. Capture the selected window identity as it moves. Keep encoded dimensions fixed on resize, fitting changed content proportionally.

A region is an adjustable rectangle in physical pixels relative to one monitor. Handle 100/150/200 percent scaling, negative desktop coordinates, portrait monitors, odd selection dimensions and monitor disconnection.

Initially stop and attempt to save when the target closes, minimizes or becomes unavailable. Static content remains valid. Show a framing preview. Landscape fits proportionally with padding. Vertical has an explicit adjustable 9:16 guide. Respect encoder dimension constraints.

Acceptance: saved content matches confirmation across mixed-DPI monitors; window resize preserves proportions.

## 4. Output presets

| Preset | Dimensions and rate | Initial video bitrate |
| --- | --- | --- |
| Landscape | 1920 x 1080 at 30 fps | 8 Mbps |
| Vertical | 1080 x 1920 at 30 fps | 6 Mbps |
| Small file | 1280 x 720 at 30 fps | 3 Mbps |

Use H.264 MP4 and AAC at 48 kHz with narration. Preserve proportions and use SDR output. Validate text, scrolling and motion. Bitrate is an initial setting, and size varies. HDR needs tested SDR conversion or a clear unsupported-source message.

## 5. Microphone

Enumerate microphones. Offer an explicitly requested brief level check and release it on dismissal. Disabled means the audio capture stream is closed.

Use a common monotonic timeline for video and audio, resample as necessary and handle drift. Report denied access before recording. Stop and attempt to save if the chosen microphone disconnects.

Acceptance: audio/video offset within 100 ms at the beginning and end of a 30-minute recording. Test built-in and USB devices where available.

## 6. Cursor and clicks

Capture button events only during recording. Timestamp and transform screen positions into capture and output coordinates. Composite effects into encoded frames. Bound active effects and expire them promptly.

Support left/right clicks, clipping at region edges, ignoring off-source clicks, rejecting unrelated-window clicks in window mode, and avoiding duplicate cursors. Animate on otherwise static content. Keep mouse callbacks short and draw in the video pipeline.

Acceptance: correct positions and timing for all modes, mixed DPI and vertical framing.

## 7. Saving and failure behavior

States: Ready -> Selecting -> Recording -> Saving -> Ready.

Stop ends acquisition, finishes pending encode/audio work, finalizes MP4 and releases resources. Show Saving until done. Write a temporary name, rename only after success, use unique names and check destination errors.

Handle disk full, encoder failure, repeated stop and closing while recording. Normal close saves before exit. Preserve and identify incomplete files after unexpected termination. Promise recovery only after testing it.

Acceptance: completed files play and seek, failures are actionable, and resources are released.

## 8. Qualification and release

Check:
- Three repeated recordings and post-record idle measurements.
- Thirty-minute static and high-motion sessions.
- Monitor, covered window and region recording.
- Mixed DPI, negative origins and portrait display.
- Window move, resize, minimize and close.
- Microphone off/on/unavailable/disconnected and long-session drift.
- Clicks inside/outside capture.
- Unwritable destination and disk full.
- Session lock, sleep/resume and display disconnection.
- Codec, dimensions, duration, playback and seeking for every preset.

Deliver release executable, usage guide and measured resource report. Keep captured videos and machine-specific build artifacts out of Git.

## Build order and progress

1. Engine prototype and short recording/memory checks — implemented and independently verified. Long-session stability, GPU memory and complete performance qualification remain pending.
2. Native control window and save lifecycle — pending.
3. Region selection and framing — pending.
4. Microphone synchronization — pending.
5. Cursor effects — pending.
6. Full failure tests, resource qualification and packaging — pending.

The current implementation request starts with milestone 1. Longer qualification and later features must remain explicitly pending until tested.

## References

- https://github.com/NiiightmareXD/windows-capture/tree/c7d106448eb9d9b251345c39047711e1cd408ae2
- https://learn.microsoft.com/en-us/windows/uwp/audio-video-camera/screen-capture
- https://learn.microsoft.com/en-us/windows/win32/medfound/sink-writer
- https://learn.microsoft.com/en-us/windows/win32/medfound/mpeg-4-file-sink
- https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc
- https://support.google.com/youtube/answer/1722171
- https://developers.facebook.com/documentation/video-api/guides/reels-publishing
