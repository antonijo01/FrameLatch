# Glass interface

November 18, 2026. Redesign of the native control surface: translucent dark glass, a wider layout and a restyled settings page. The recording engine is unchanged.

## Design

- **Material.** Windows desktop acrylic (`DWMWA_SYSTEMBACKDROP_TYPE` = `DWMSBT_TRANSIENTWINDOW`) is extended over the whole client. A dark wash (RGB 9/10/14 at 30 % opacity) keeps white text legible over bright windows while the desktop stays visible.
- **Unfocused translucency.** Windows renders this acrylic solid while a window is inactive, and a recorder panel usually sits unfocused beside the app being recorded. `WM_NCACTIVATE` is therefore answered as active while acrylic is in use. Only the frame rendering changes; activation and focus behave normally, and the title bar always looks active.
- **Cards.** Every surface uses one recipe adapted from the MIT-licensed [liquid-glass](https://github.com/ziuus/liquid-glass) shader, without sampling the backdrop: tinted body, a sheen over the top 45 %, a 1 px hairline and a diagonal specular rim lit from the top-left.
- **Hierarchy.** The selected Screen / Window / Area segment is a near-white pill with dark content. Red is reserved for Start recording and Stop & save, and Cancel is neutral. The microphone switch turns green when on, and its knob is 82 % of the track height.
- **Layout.** The main panel client area is 544 × 408 logical pixels, with 24 px gutters and an 8 px rhythm. Settings is 544 × 416 and the toolbar 464 × 62. The frame is computed per DPI with `AdjustWindowRectExForDpi`.
- **Main panel.** It has the source modes and a source picker whose icon matches the mode. Below them are the source detail line and two cards: Microphone and Settings. An output card shows the save folder, quality and click effect, with Change. Then come Start recording and a Ctrl+Shift+F9 hint.
- **Settings.** The page has Back and a heading, labelled glass dropdowns for quality, microphone device and mouse clicks, and a save-folder card. The card shows the full path and opens the Windows folder picker. The underlying edit control stays hidden as the path store.
- **Fallback.** High contrast, Windows transparency effects turned off, or a failed acrylic request all restore the opaque GDI interface with system colors. Battery Saver and Energy Saver can also turn transparency off system-wide, and the backdrop is then solid.

## Implementation

`src/glass.rs` draws with Direct2D, using a software DC render target, into a 32-bit premultiplied `BeginBufferedPaint` buffer. GDI writes zero alpha, which DWM shows as transparent or additive on glass, so GDI is not used for these surfaces. Text uses DirectWrite with grayscale antialiasing.

Child controls cannot be transparent. Each owner-drawn button, label and dropdown therefore repaints the part of the parent surface it covers before drawing itself (`paint_child`). Dropdowns paint their closed state in a subclass; their drop-down lists remain native popups.

A combo box recomputes its selection-field height whenever it is resized. `set_picker_height` runs after layout, and again on font, theme and DPI changes.

## Validation

On one Windows 11 display (build 26300) at 200 % scaling, on AC power, screenshots were taken over a bright striped test window while FrameLatch was unfocused. The panel, settings and countdown toolbar were all translucent. A focused-state check was not possible because automation could not take foreground focus. The documented acrylic alone, and the undocumented accent blur and accent acrylic, all rendered solid or black when unfocused on this build.

Exercised through window messages:
- Screen and Window modes
- Settings open and Back
- Countdown start and Cancel, which returned to the panel with Start recording / Ready

No recording was made in this pass.

Idle private memory after launch was 13.4 MiB, working set 34.9 MiB. The previous GDI interface used about 3.2 MiB private. A hardware Direct2D target measured 43 MiB and was rejected in favour of the software target.

Formatting, Clippy with warnings denied, release build and eleven unit tests pass.

Not verified:
- appearance of the open drop-down lists on glass
- keyboard focus rings
- live high-contrast switching
- mixed-DPI or multiple monitors
- a full recording through the new interface
