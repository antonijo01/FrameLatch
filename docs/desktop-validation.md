# Desktop validation — November 15, 2026

The current glass interface is described in [glass-ui.md](glass-ui.md). The observations below cover the initial interface and the recording engine.

Development machine: Windows 11, one 3000 × 1876 display at 200% scaling, Intel Arc 140T. Tests use actual Windows mouse/keyboard input, native controls, captured output and FFprobe/FFmpeg inspection. Numeric observations are not promises for other hardware.

## Tests completed during implementation

- Launch native GUI, choose Small quality, edit destination, cancel countdown through its button, start recording and stop with Ctrl+Shift+F9.
- Open native folder chooser and cancel it.
- Capture a known test picture; inspect decoded output for upright orientation, ordinary cursor and exclusion of FrameLatch controls.
- Select a microphone in the GUI, record audible test tones, stop and inspect both streams: H.264 1150 × 720, 99 frames / 3.3 seconds; AAC 48 kHz stereo / 3.281646 seconds. Audio was non-silent. This proves basic narration, not long-term synchronization.
- Timed ten-minute monitor recording completed successfully: H.264 1726 × 1080, 30 fps, 18,000 frames, exactly 600 seconds. Whole recorder process-tree private memory after initial warmup ranged from 153.66 to 230.37 MiB, ending at 201.91 MiB. The desktop changed during the test; this is a mixed-content test, not a controlled static benchmark. Other applications and a second recorder overlapped part of the run.
- A planned thirty-minute microphone test was stopped after approximately 251 seconds when the user closed the test picture and asked whether recording was still running. Its observed process-tree private memory peaked at 226.03 MiB. No synchronization or complete-file claim is made for this interrupted test. The controller was terminated, and its Windows job also terminated the worker.
- Confirmed all qualification recorder processes exited. Deleted both long-test videos, including the interrupted partial file, and the inspected short test media.
- Native GUI snapshot idle test: twelve samples over 60 seconds after a three-second warmup measured 2.648–2.707 MiB private memory and 18.801–18.844 MiB working set. Process CPU time increased by 0.03125 seconds, approximately 0.00325% of total machine CPU capacity. This snapshot preceded final area/click/preset changes.
- Final-feature GUI after several recordings: twelve samples over 60 seconds measured 3.047–3.199 MiB private memory and 23.262–23.348 MiB working set. CPU time did not increase at the measurement resolution. Private memory returned within 1 MiB of the earlier idle baseline. This build preceded only the final missing-file error-message change. The app closed normally afterward.

## Final feature checks

| Check | Observed result |
| --- | --- |
| Countdown and area cancellation | Escape cancelled both; cancel button also exercised |
| Area crop and ring | 800 × 500 output, 2.566633 seconds; decoded rings centered at approximately (400,250) and (700,400), exactly matching monitor-local click positions |
| Ring lifetime | Ring appeared on successive static frames and expired; no accumulation |
| Window capture while covered by controller | Correct full window, H.264 1040 × 720; controller absent |
| Window microphone capture | AAC 48 kHz, 1.045312 seconds beside 1.066633-second video; near-silent input in this particular environment, not a narration-quality assertion |
| Portrait Small | H.264 720 × 1280, 2.8 seconds; full source fit with black bars |
| Portrait Standard | H.264 1080 × 1920, 1.366633 seconds |
| Ripple | Decoded marker expanded from approximately 20 to 54 pixels across successive frames, then expired |
| Close FrameLatch during capture | Finalized portrait output and exited; no worker remained |
| Move recorded window | Dragged window by (200,100) desktop pixels; click marker followed the new local position |
| Click a covering window | No marker appeared in the captured source during that interval |
| Close recorded window | Reported capture ending unexpectedly; preserved playable 0.8-second / 24-frame partial MP4 |
| Minimize recorded window | Reported minimized source and preserved playable 3.066633-second partial MP4 |
| Shortcut | Ctrl+Shift+F9 stopped active captures; deliberately occupied shortcut produced a warning, and the visible Stop button still saved successfully |
| Keyboard | Tab moved from folder edit to Browse; Shift+Tab moved back |
| Playback/reveal | Open video launched Windows Media Player; Show in folder opened Explorer at the saved recording |
| Invalid destination | Existing regular file used as folder was rejected without capture; stale countdown status found and corrected |
| Settings | Quality persisted across reopen; unknown-version settings produced a notice and safe quality/microphone/click defaults; original settings restored after test |
| Microphone Off | Silent recordings contained only the H.264 video stream |

The independent reviewer checked output reservation/no-replace publication, periodic free-space checks, stop/finalization, audio timestamp handling, crop bounds, window click filtering and settings validation. Five automated tests cover dimensions, portrait fit, negative-monitor crop clamping, audio overlap/gap/stop clipping and no-replace publication. Formatting, release build and Clippy with warnings denied pass.

## Testing procedure correction

Closing a separate test-picture window does not stop a CLI timed recording. Announce short desktop recording tests before starting them and explicitly confirm when they finish. Avoid unattended long desktop recordings during interactive use. Normal FrameLatch window close was confirmed to request Stop and finalize the file.

The first area/hotkey attempt used an incorrect automation wait: it matched the countdown text before capture started. Corrected the wait to require active recording and repeated the checks successfully. Window source tests must select by title, not assume enumeration index; one incorrectly selected source run was discarded and repeated. These were test-harness errors, not accepted product failures.

## Outstanding qualification

The thirty-minute audio synchronization gate remains unpassed. Mixed-DPI/multiple displays, physical microphone removal, microphone permission denial, 44.1 kHz hardware, actual disk-full conditions, redirected Videos folders and 100%/150% display scaling were not exercised. No controlled ten-minute-motion benchmark was completed. GPU allocations and exact hardware-encoder selection are not measured by private-memory readings. Window resize fails safely by source inspection but was not separately exercised. The final missing-file Open video message was source/build checked after the test revealed the earlier silent failure; it was not separately retested in the desktop.

Generated capture content is disposable and must be removed after inspection. Preserve user recordings and retain numeric results only.
