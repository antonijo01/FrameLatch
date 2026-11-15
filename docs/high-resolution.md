# High-resolution recording

Adds High · 1440p (2560 × 1440 maximum), Ultra · 4K (3840 × 2160 maximum) and Original to the native quality picker. Standard 1080p remains the default. Existing saved quality indices and portrait presets retain their meaning.

Landscape presets preserve source aspect ratio, fit inside the selected maximum, and never enlarge smaller captures. Original preserves screen/window/selected-area dimensions, rounded down to even dimensions for H.264 compatibility. For the development display (3000 × 1876), 4K and Original both retain 3000 × 1876; a 4K setting cannot create extra source detail. Original retains the source shape, including tall regions, rather than forcing a landscape canvas.

Higher output resolution requires more encoding work and storage. Idle behavior remains unchanged: capture and microphone run only during recording. Encoding remains H.264 at 30 fps. Legacy portrait presets continue to fit the source into their fixed portrait canvas.

## Verification

Real Windows mouse/keyboard testing selected each new preset, recorded a short video, stopped it through the toolbar, and verified complete error-free decoding with FFmpeg. FFprobe confirmed H.264 at 30 fps:

| Selected quality/source | Actual output | Duration | File bytes |
| --- | --- | --- | --- |
| High / 3000 × 1876 screen | 2302 × 1440 | 2.600 s | 2,557,891 |
| Ultra / 3000 × 1876 screen | 3000 × 1876 | 2.733 s | 4,705,111 |
| Original / 3000 × 1876 screen | 3000 × 1876 | 2.800 s | 4,886,047 |
| Original / 801 × 501 area | 800 × 500 | 2.700 s | 1,700,375 |
| Ultra / 801 × 501 area | 800 × 500 | 2.733 s | 1,733,704 |

All three new choices survived close/relaunch without invalid-settings warnings. The visible dropdown was checked at 200% scaling. The test recordings were deleted promptly after decoding, original preferences restored, and test windows closed.

Sampled peak controller-plus-worker private memory was approximately 250 MiB for High screen capture, 311 MiB for Ultra/Original screen capture, and 182 MiB for small areas. These are short test observations, excluding GPU memory; they do not establish long-session stability. A fresh idle process after restart used 3.00 MiB private and 20.26 MiB working set, with no children. No new idle work or dependencies were added.

New presets request a bitrate proportional to actual output pixels: 8 Mbps minimum, 32 Mbps for 3840 × 2160, capped at 80 Mbps for larger Original captures. Actual file size varies with content and encoder behavior. Legacy 1080p/720p bitrates remain 8/3 Mbps.

Formatting, Clippy with warnings denied, release build and eleven tests passed. Tests cover legacy dimensions, stable persisted indices, worker protocol round trips, 3840 × 2160 sizing, source aspect, smaller areas, odd native dimensions and bitrate scaling. Independent review found no routing or encoding bug; a misleading Landscape summary for source-aspect presets was corrected to Up to [resolution].

A physical 3840 × 2160 display was unavailable: exact 4K sizing is covered by unit tests, while live capture was verified up to 3000 × 1876. Very large Original sources depend on the installed encoder's limits; long-duration high-resolution recording and GPU memory remain unqualified.
