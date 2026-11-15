# Reference-led UI refinement

November 15, 2026. Follow-up to the compact dark panel, based on the user's supplied visual reference. This work changes the native interface, not the recording engine.

## Scope

- A shared segmented Screen / Window / Area selector, with small icons and an unmistakable selected state.
- A clearer source selector and secondary source information. Area mode must display the selected crop dimensions and offer Change, including after a successful recording.
- A recognizable microphone switch with explicit On/Off text and native keyboard operation.
- Consistent charcoal surfaces and less prominent borders; retain subtle Windows material treatment without a disconnected gray header band.
- Separate action, label and secondary typography; DPI-scaled spacing and corner radii.
- Recording-circle icon on the main action, without an unimplemented shortcut hint.
- Short save-location summary with a Change action. Keep complete preferences in settings.
- A compact conditional saved-result row; preserve the existing small recording toolbar.

Keep the custom FrameLatch logo. Do not introduce a PRO badge, an invented version, decorative animation, live previews, a browser runtime or per-setting accent colors. Quality/orientation/click details remain a readable summary unless they have an explicit action.

## Verification contract

Use actual Windows mouse and keyboard input to inspect the panel, dropdowns, settings and focus. Exercise source modes, area cancellation/reselection, microphone switching, folder-change cancellation, countdown, recording toolbar, Save, result actions and retained area dimensions. Announce short recordings before starting them, inspect the resulting files and delete generated capture content afterward.

Run formatting, Clippy, release compilation, meaningful source/area-state tests and independent review. Measure idle resource use after recording. Preserve current preferences and user recordings. Document unavailable hardware/accessibility tests as unverified.

## Implementation and validation

Implemented in the existing Win32/Rust interface with native controls and small GDI-drawn icons. No new dependencies, browser runtime, live preview or idle timer. The client is solid charcoal; Windows material is confined to the title bar. Main window: 450 × 355 logical pixels, observed as 900 × 710 at 200% scaling. The existing compact toolbar remains.

Actual mouse and keyboard tests on Windows 11 at 200% scaling passed:

- Screen/Window/Area selection and contextual source list; microphone click and Space toggles.
- Folder Change opens the Windows picker; Escape cancels it. Settings output-folder edit and Back work.
- Cancelling area selection displays a missing-area message and blocks Start. Reselecting an 800 × 500 crop displays its exact dimensions and Change action.
- Countdown Cancel returns to the panel. Stop saves and returns to the main panel. Selected-area dimensions remain visible alongside the saved-result actions.
- Open video launched the media player; Show in folder opened the test directory.
- Area recording: H.264, 800 × 500, 2.567 seconds, no audio stream with microphone off.
- Window recording: explicitly selected the test surface by name; H.264, 1560 × 1080, 2.603 seconds, with AAC microphone audio.
- Both MP4s decoded completely without FFmpeg errors; extracted frames showed the intended test surface/crop. This checks audio stream creation, not long-duration synchronization or subjective microphone quality.

All generated test videos were deleted after inspection. User recordings were preserved, original preferences restored, and test recorder/fixture windows closed.

Resource observations (CPU-process memory only; GPU memory excluded): fresh launch used 3.21–3.22 MiB private memory and 22.42–22.52 MiB working set. After recordings, folder picker and result actions, a 60.28-second idle check stayed at 12.84–13.19 MiB private and 64.45–64.74 MiB working set, consumed 0.046875 CPU seconds, and had zero child processes throughout. The higher post-interaction footprint is reported separately; its allocation source was not isolated, so it is not attributed to a particular feature. No growth was observed during this short idle sample.

Formatting, Clippy with warnings denied, release build and nine tests pass. Independent review checked source/area state, microphone, folder and countdown flows, and identified outdated Mica-header wording; documentation and helper/test naming were corrected to describe the intended solid client. No outstanding functional finding remained.

Mixed-DPI/multiple-monitor hardware, live high-contrast switching and long recordings were not requalified in this UI pass. Earlier engine validation remains documented separately.
