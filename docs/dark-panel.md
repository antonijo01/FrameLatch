> Historical validation of the first dark panel. The current refinement uses a solid charcoal client and titlebar-only material; see [reference-ui-refinement.md](reference-ui-refinement.md). Earlier dimensions and measurements below describe that earlier build.

# Dark panel and recording toolbar

November 15, 2026. Redesign of the native Windows control surface; the recording engine remains unchanged.

## Design contract

Visual direction: a compact charcoal panel with restrained Windows Mica treatment, readable controls and one prominent recording action. Use native window controls and Segoe UI. Avoid continuous animation, live previews and a browser runtime.

The first screen presents Screen / Window / Area, a contextual target selector, microphone state, a concise quality/effect summary and Start recording. Preferences and the full output path live in an expandable section. Finished-file actions appear only after a recording succeeds.

The same window switches to a compact toolbar for starting, recording and saving. It clearly identifies the current state, microphone state and elapsed time, and provides Stop & save. Completion or failure restores the main panel. Closing during capture retains graceful Stop/finalization behavior. Selecting Area never silently falls back to recording the whole monitor after cancellation.

Interaction priorities: explicit source selection; clear countdown cancellation; an unmistakable recording state; a visible stop action; full keyboard access. Glass is a restrained backdrop treatment, not a transparency effect applied to text or video previews.

## Efficiency and compatibility

- Reuse native Win32 controls and the existing recording worker.
- Request DWM Mica on supported Windows versions, with a solid dark fallback. Microsoft's DWM system-backdrop enum requires Windows 11 build 22621 or later.
- Respect high-contrast mode and preserve visible keyboard focus.
- Keep capture exclusion for the controller/toolbar.
- Measure whole-process idle private memory, working set and CPU; compare with the preceding GUI's approximately 2.7–3.2 MiB private / 19–23 MiB working-set observations.
- Do not create ongoing capture, microphone or preview resources while idle.

## Implemented appearance and identity

At 200% scaling, the main window is 900 × 700 physical pixels (450 × 350 logical), settings is 900 × 780, and the countdown/recording toolbar is 920 × 220. The previous control window was approximately 1020 × 1100. The toolbar alone is topmost; the restored panel returns to normal window stacking.

The Mica header covers the source controls in the main panel and only the Back row in settings. Solid charcoal beneath the content preserves contrast. Backdrop painting is enabled only when the DWM calls succeed. Owner-drawn buttons retain native control names, keyboard navigation and visible focus. Native dropdowns keep their selection behavior with dark arrow/frame painting.

The FrameLatch mark uses white capture-frame corners and a red recording dot on a charcoal rounded square. It is embedded in the PE executable and loaded for both small and large window icons. `assets/framelatch.svg` is the vector artwork; `assets/render_icon.py` mirrors its geometry to reproduce the ICO. Included sizes: 16, 20, 24, 32, 40, 48, 64, 128 and 256 pixels. No icon rendering occurs at runtime.

## Validation

Actual Windows mouse/keyboard tests on the development machine exercised:

- Main panel, settings, native dropdowns, compact summaries and microphone toggle; edited settings survived reopening.
- Screen / Window / Area selection and contextual source lists.
- Escape and Alt+F4 canceling the area overlay, followed by focus returning to FrameLatch. Attempting to Record without an area produced an explicit selection prompt, not full-screen capture.
- Closing the owner while the selection overlay was active: process exited cleanly. This and the overlay Alt+F4 behavior were bugs found and fixed during independent review.
- Repeated countdown cancellation, with the compact toolbar returning to the panel.
- Keyboard Tab and Shift+Tab between folder edit and Browse, with visible focus.
- Invalid output folder: countdown ended in a useful error and the full panel returned to Ready.
- Area recording: selected 796 × 502 pixels, output H.264 796 × 502 for 1.7 seconds. Red ring centered at approximately (400,250), matching the actual source click. Toolbar displayed elapsed time and Mic off; Stop & save showed Saving and then restored the panel with finished-file actions.
- Window recording with portrait output and microphone: H.264 720 × 1280 for 1.166633 seconds plus AAC 48 kHz for 1.173312 seconds. Toolbar displayed Mic on; Ctrl+Shift+F9 stopped and saved. This validates stream creation, not long-duration audio synchronization.
- Open video launched the Windows player; Show in folder opened Explorer; deleting that test file then using Open video produced a moved/deleted message.
- Actual title-bar icon visually matched the custom artwork. Embedded icon resource and all nine ICO sizes were checked; regenerating the ICO reproduced it byte-for-byte.

Formatting, Clippy with warnings denied, release build and eight tests pass. New tests cover view precedence, an absent area blocking recording, and view-specific material margins. Independent review checked modal selection ownership, timer lifecycle, icon resource ownership, dark/high-contrast paths and combo subclass GDI cleanup. A suspected Mica issue was rejected after checking Microsoft's black-brush/transparent extended-frame guidance and the actual composed header.

The final event-driven-timer build recorded and saved H.264 1270 × 720, 31 frames / 1.033300 seconds. After the idle measurement, a second recording started normally; closing its toolbar finalized 21 frames / 0.7 seconds and the process exited cleanly. Both files were inspected and deleted.

Twelve post-recording samples over 60.41 seconds measured 2.973–3.344 MiB private memory and 20.941–23.305 MiB working set. CPU time increased by 0.03125 seconds, approximately 0.00323% of total machine CPU capacity. No recording child process remained during the idle interval. Handle count ranged from 264 to 300 and ended at 299; this short observation is not a long-duration leak test. The release executable is 939,008 bytes, including the embedded icon, with no added runtime dependency.

High-contrast mode and other DPI/multi-monitor configurations are source-reviewed but not physically exercised. The earlier long-duration audio and GPU-allocation qualification limits still apply. All recording tests were short and announced; generated media was deleted after inspection. Only the final UI/logo previews are retained as user-facing deliverables.

## References

- [Windows Mica guidance](https://learn.microsoft.com/en-us/windows/apps/design/style/mica)
- [DWM system backdrops](https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/ne-dwmapi-dwm_systembackdrop_type)
- [Win32 dark themes](https://learn.microsoft.com/en-us/windows/apps/desktop/modernize/apply-windows-themes)
- [High-contrast handling](https://learn.microsoft.com/en-us/windows/win32/winauto/high-contrast-parameter)
- [Screen Studio recording picker](https://screen.studio/guide/new-recording)
- [EarTrumpet](https://eartrumpet.app/)

These references guide appearance and interaction; their technology stacks are not dependencies of FrameLatch.
