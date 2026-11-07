# Anti-aliasing and motion blur

Both are settings (Options > Feedback list: "Anti-aliasing", "Motion blur"), saved in the settings file as `anti_aliasing`
(0 off, 1 4x, 2 8x, 3 16x) and `motion_blur` (0 off, 1 subtle, 2 moderate, 3 intense). Defaults: 4x and subtle. The renderer
itself starts with both off, so tests and tools that do not set them draw as before.

## Anti-aliasing

* **4x, 8x**: that many samples a pixel (wgpu MSAA). Colour and depth are multisampled and resolved into the finished picture.
  8 needs the adapter's per-format support (`TEXTURE_ADAPTER_SPECIFIC_FORMAT_FEATURES`); where the card has less, 8x is 4x.
* **16x**: no card has 16 samples at this format, so it is 8x plus a short FXAA pass (WGSL, `fs_fxaa` in `gpu/src/lib.rs`) on
  the finished picture. That also softens edges samples cannot reach (cut-out leaves, hair).
* Pipelines are rebuilt for the sample count when the level changes (`make_pipelines`). The UI is drawn after, into the window,
  so text is never smoothed or blurred.

## Motion blur

* A post pass after the scene. Each pixel's depth and the inverse of this frame's view-projection give its world point. The
  previous frame's view-projection says where that point was. The picture is smeared along the way it came, 12 taps.
* Camera movement only. Bodies that move on their own are not smeared separately.
* The length is the movement per second times the shutter (subtle 10 ms, moderate 20 ms, intense 36 ms), so 30 and 144 frames a
  second look alike. It is capped at 1.2%, 2.6% and 5% of the picture width.
* None on: the first frame after `set_static_scene` (a cut), a frame that took over 0.25 s, a menu, a loading screen, a paused
  game (`set_motion_blur_suspended`), or figure-only screens.

`cargo run --release --example aa_shots -- <module> <out_dir> [w] [h] [frames]` writes the frames and prints what each costs.
