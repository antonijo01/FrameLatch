# FrameLatch everyday-use implementation plan

Updated November 15, 2026. This document defines the next implementation units. The broader requirements remain in implementation-plan.md. Features below are planned unless explicitly marked implemented.

## Outcome and delivery order

Open FrameLatch, choose what to show, optionally enable narration, record, stop with a shortcut, and find the saved MP4 immediately.

| Delivery | Features | Relative effort |
| --- | --- | --- |
| A | Native window, monitor choice, countdown, Record/Stop, shortcut, remembered folder, dated filenames, open/reveal file, Standard/Small quality | Medium overall; individual controls are small |
| B | Microphone toggle, device choice and synchronized narration | Medium; timing is the main risk |
| C | Region selection and red click ring | Medium; shared coordinate transforms |
| D | Window source, vertical framing, red ripple | Separate focused additions |

These are priorities, not calendar estimates. Pause/resume follows once timestamp handling is established. The original window/vertical/ripple requirements remain in the backlog. Automatic upload and editing would require separate scope.

## Verified starting point

At commit 3634114, src/main.rs handles CLI dispatch and src/record.rs implements capture, scaling, encoding and worker lifetime. Dependencies include windows-capture 2.0.1 and windows 0.62.2.

Implemented: monitor selection, silent H.264 MP4, proportional fit within 1920 x 1080 at 30 fps, one pending image, a short-lived worker, Windows job protection and startup pipe handshake.

Current gaps: recording ends after a fixed duration, the controller blocks waiting for its worker, and there is no interactive Stop or structured status channel. Capture uses CPU readback and nearest-neighbor scaling. Fixed-frame output catches up briefly after delay and fails if more than one second behind.

Observed on the development machine: 3.29 MiB private memory between worker sessions and 195–225 MiB during short recordings including helpers. GUI memory, GPU memory, CPU budget and thirty-minute stability remain unverified. Existing test media was deleted after inspection.

## Implementation structure

Reuse the recording engine and the pinned Windows bindings. Use native Win32 controls and a blocking message loop while idle. Add Windows API features as needed.

Introduce modules only when used: ui.rs for controls, session.rs for worker control, settings.rs for preferences/output allocation, audio.rs for narration, selection.rs for rectangle selection, and effects.rs for clicks. Retain record.rs for capture/encoding and main.rs for GUI/CLI dispatch.

Share one RecordingOptions structure between GUI and CLI containing source identity, crop, quality, output, microphone and click mode. CLI remains useful for repeatable verification. Small serde/serde_json dependencies can support settings and bounded newline-delimited worker messages. Keep diagnostics on stderr and drain both output pipes.

## Unit 1 — responsive recording and graceful Stop

Retain worker stdin after the existing handshake. Accept Start/Stop commands on a blocking reader thread and signal the recorder through a stop primitive. Report Ready, Started, Saved(path), and Error(message, partial_path). Send no per-frame progress messages.

The controller reads events and waits for child exit off the UI thread. Start time is confirmed by Started. Stop interrupts initial-frame waiting and scheduler waits, stops acquisition, finishes encoding and finalizes MP4. Timed CLI mode uses the same stop path. Duplicate Stop is harmless.

Normal window close requests Stop and displays Saving until completion. Keep the job alive until the child exits. Unexpected controller exit retains kill-on-close protection. EOF on the command pipe requests graceful stop when possible. A blocked encoder gets a documented timeout and an explicit force-close option identifying the output as incomplete.

Acceptance: stop on static content, during startup and active recording; double Stop; close while saving; startup failure; controller termination; successful final rename before Saved. Preserve existing timed CLI commands.

## Unit 2 — small native window

Controls: monitor dropdown, quality dropdown, folder chooser, Record/Stop, elapsed status, Open video and Show in folder. Microphone controls arrive in unit 4. Standard controls provide keyboard navigation and accessible labels. If Windows supplies an empty display name, show monitor number and resolution, matching the CLI fallback.

States: Idle, Selecting, Countdown, Starting, Recording, Saving, Error. Disable source/quality controls while active. A cancellable three-second countdown uses UI timers. Escape cancels countdown and area selection.

Proposed Stop shortcut is Ctrl+Shift+F9, registered only during an active session using RegisterHotKey and MOD_NOREPEAT. If occupied, show the conflict and keep the visible Stop control usable. Unregister on completion.

Use WDA_EXCLUDEFROMCAPTURE for the controller and verify the actual video. If exclusion fails, minimize only after confirming a usable shortcut. Hide selection controls before capture starts. Elapsed time updates once per second from Started. Use STA COM on the UI thread for shell dialogs and MTA in media work.

Acceptance: double-click startup, keyboard operation, 100/150/200 percent scaling, occupied shortcut, cancelled countdown, responsive Saving, and recorder controls absent from saved video.

## Unit 3 — files, settings and compression choices

Resolve the Windows Videos known folder and default to its FrameLatch subfolder. Folder selection uses IFileDialog with FOS_PICKFOLDERS. Support redirected folders, spaces and Unicode.

Generate FrameLatch-YYYY-MM-DD-HHMMSS.mp4 with a collision suffix. Atomically reserve unique temporary output, including the partial-file name, and publish without replacing an existing final file. Concurrent recordings must not overwrite each other.

Store versioned settings in LocalAppData/FrameLatch/settings.json with defaults. Replace settings atomically. Invalid settings produce a notice and usable defaults. Persist device identity rather than list index and revalidate it at Start.

Use ShellExecuteW to open the finished video and SHOpenFolderAndSelectItems to reveal it. Enable these actions only after success. Report moved/deleted files clearly.

| Quality | Output cap | Frame rate | Initial bitrate |
| --- | --- | --- | --- |
| Standard | Fit proportionally within 1920 x 1080 | 30 fps | 8 Mbps |
| Small | Fit proportionally within 1280 x 720 | 30 fps | 3 Mbps |

Approximate video size is 60 or 22.5 MB per minute before audio; actual size varies. Exact padded canvases arrive with framing controls. Check text readability and scrolling before accepting either preset.

Check output access/free space before Start and periodically during recording. Keep an explicit small free-space reserve for attempted finalization and handle actual write errors. Preserve incomplete user recordings for review.

Acceptance: same-second filenames, concurrent reservation, redirected Videos, spaced/Unicode paths, corrupt settings, missing device, unwritable destination and simulated low disk space.

## Unit 4 — microphone narration

Add on/off and input-device selection before recording. Keep input closed when idle or disabled. An explicitly requested level check opens it temporarily and releases it on dismissal.

Use event-driven shared-mode WASAPI through existing Windows bindings. Study wasapi-rs examples; add its wrapper only if it materially reduces code and supplies the required timestamps. Configure an AAC stream in the existing sink before BeginWriting. Negotiate/resample input to 48 kHz; preserve mono narration where appropriate.

Bound audio buffering by time, initially 250 ms, and report overflow. Audio callbacks must not wait indefinitely on a slow encoder. Use a common recording epoch and convert audio device/QPC positions and video timestamps into it. Callback arrival time alone is insufficient. Handle delayed first samples, discontinuities and drift, inserting silence only for known bounded gaps.

Denied access or unavailable input is reported before recording. Device removal stops and attempts to save with an explanation. A changed system default does not silently replace a selected device.

Acceptance: no input stream when disabled, valid AAC when enabled, built-in and USB microphones where available, silence, denied permission, disconnect, and less than 100 ms audio/video offset at the start and end of a thirty-minute run.

## Unit 5 — region selection

Reuse monitor capture and the resize stage. Present a dimmed overlay on the selected monitor with a draggable rectangle, confirm and cancel. Redrawing is sufficient for initial adjustment.

Set per-monitor-v2 DPI awareness before window creation. Store the rectangle in monitor-local physical pixels, clamp bounds, validate nonzero size and align encoded dimensions as needed. Keep source width/height with the frame; a resize with equal byte count must still be detected.

Crop before resizing. Remove selection UI before capture. Revalidate geometry at Start and stop safely if it changes during recording. A region remains fixed relative to its chosen monitor.

Acceptance: known grid image, odd dimensions, negative desktop coordinates, mixed DPI, portrait display, cancellation, disconnect and correct saved boundary pixels.

## Unit 6 — red click highlights

Implement Off and Red ring first; Red ripple follows through the same renderer. Capture the ordinary Windows cursor exactly once.

Install the low-level mouse hook only during recording. Its callback copies position, button and monotonic timestamp to a bounded queue and returns immediately. Render into output pixels before bottom-up encoder row conversion.

Use the same crop/scale transform as the picture, ignore off-source clicks and clip effects at edges. Proposed defaults: 300 ms duration, three-pixel outline, modest output-pixel radius. Cap simultaneous effects at eight and expire the oldest on overflow. Include left/right button presses.

Composite each output frame from a clean latest image so rings cannot accumulate. Animate at the output cadence on static content. Release hooks and effect queues at Stop.

Acceptance: correct positions in decoded video for mixed DPI, region and whole-monitor modes; rapid clicks; boundaries; off-source clicks; static content; clean teardown.

## Unit 7 — remaining agreed controls

Window source uses a system-picker capture item and retains its identity during movement. Preserve fixed encoded dimensions and proportional fit on resize. Explicitly define child-dialog inclusion. Test covered, moved, resized, minimized and closed windows. Filter click effects to the selected window and intended children.

Vertical output adds a 9:16 framing guide and 1080 x 1920 at 30 fps/6 Mbps. Show exactly which source content is retained. Define fit/upscale behavior for smaller sources. Exact Landscape canvas adds proportional padding to 1920 x 1080. Distinguish these framing options from proportional quality caps.

## Efficiency and acceptance gates

Initial GUI targets: below 30 MiB private memory idle, total-machine CPU below 0.1 percent averaged over sixty seconds, and post-record idle within 10 MiB of warmed baseline. Closing exits the app fully.

Measure the whole process tree. Use a provisional 300 MiB recording-private-memory budget for the existing development display at Standard quality; other resolutions need measured budgets. Record GPU dedicated/shared memory separately. These targets are unproven for the GUI.

Before delivery A, record ten minutes of static content and ten minutes of motion, measuring CPU, memory, frame timing and pending queues. Before narration delivery, run the thirty-minute synchronization test. Investigate sustained memory growth after warm-up.

Reuse scratch/frame storage and remove avoidable large copies where ownership permits. Profile before replacing the CPU pipeline. If CPU or image quality fails acceptance, improve buffer reuse/scaling first and evaluate GPU processing as a bounded follow-up.

Add regression tests for state transitions, crop transforms, timestamp conversion and filename collisions. Runtime checks cover actual capture, encoding, playback, audio, process cleanup and low-disk handling. Each unit gets build/format/Clippy checks, appropriate tests, independent verification and a small commit.

Delete generated test videos/screenshots immediately after inspection, retaining numeric results. Preserve user recordings. Keep media and build artifacts ignored.

## Research references

Checked November 15, 2026. Current local source establishes implemented behavior. External examples guide specific API usage; chosen dependencies require version compatibility checks.

- Microsoft windows-rs was checked at f37e2f9046cf9710fe00285436c4d22ad6053d72. Adapt examples to the project's pinned 0.62.2 bindings. [Windows Rust sources](https://github.com/microsoft/windows-rs/tree/f37e2f9046cf9710fe00285436c4d22ad6053d72)
- RegisterHotKey delivers WM_HOTKEY and supports MOD_NOREPEAT; handle registration failures. [Microsoft hotkey API](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-registerhotkey)
- Native folder selection and Explorer file reveal are available directly through Windows. [Folder dialog](https://learn.microsoft.com/en-us/windows/win32/shell/folder-id), [Explorer selection](https://learn.microsoft.com/en-us/windows/win32/api/shlobj_core/nf-shlobj_core-shopenfolderandselectitems)
- Verify capture exclusion for the actual app and capture mode. [Display affinity](https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-setwindowdisplayaffinity)
- WASAPI provides capture endpoints and audio clocks with explicit unit conversions. [WASAPI](https://learn.microsoft.com/en-us/windows/win32/coreaudio/wasapi), [Clock position](https://learn.microsoft.com/en-us/windows/win32/api/audioclient/nf-audioclient-iaudioclock-getposition)
- MIT-licensed wasapi-rs record example uses shared event capture and sync_channel(2). The sample writes raw audio and requires additional stop/synchronization logic and bounds on its internal deque. [Pinned recording example](https://github.com/HEnquist/wasapi-rs/blob/13cbc8042b23ddbf21e468ecd7f873e511675ab0/examples/record.rs)
- MIT-licensed Win32CaptureSample explains HWND/HMONITOR selection and free-threaded pools. [Pinned capture sample](https://github.com/robmikh/Win32CaptureSample/tree/49fefe79fd9b11025f0b5eb91783a98888516070)
- Mouse-hook callbacks must return promptly; rendering belongs in the video pipeline. [Mouse hook API](https://learn.microsoft.com/en-us/windows/win32/winmsg/lowlevelmouseproc)
