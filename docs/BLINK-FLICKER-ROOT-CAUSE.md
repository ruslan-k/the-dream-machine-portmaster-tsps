# Investigated: intermittent blink flicker over sub-cached objects

Status of the fix: **pending owner verification** (build `filters16`).

## Symptom

During idle gameplay the character's face flickers: for a few refreshes,
a rectangular block of smeared content replaces the eye area (`глитчи то
появляются, то пропадают` — the owner associated it with the character's
blinking). The same class was seen on the trousers/leg strips.

## Instrumentation that made this solvable

1. `DREAM_SCRAPE=1` — per-render FNV hash of a fixed framebuffer region
   (x 700-880, GL-y 0-380), dumped on every change. `glFinish()` before the
   readback is mandatory; without it the Mali readback races the GPU and
   fabricates a fake multi-state `churn`.
2. Exact-match verdict: a caught panel frame (kmsgrab) is compared
   byte-for-byte with the stored render dumps (`scripts/compare_panel_vs_dumps.py`,
   orientation = row flip only). Result: the panel's periodic flashes match
   the renderer's own states almost exactly (75-94 changed px of 68 400) —
   **the corruption is produced by the renderer**, not the present path.
3. `DREAM_FILTER_TRACE=1` — staged PPM dumps of every offscreen cache
   entry (`A-object` raw content / `B-copy` scratch copy / `C-final`
   composite result), armed on content-change frames (blink frames),
   `glFinish()` before each readback.

## What the dumps showed

- The raw content of the animated entries is byte-identical across
  hundreds of blinks (`A-object` constant).
- **The composite result of the same raw content is different every
  time**: measuring the eye-strip object (97x34, drop-shadow filter), the
  same raw strip rendered *unchanged* on some blinks, *darkened* on
  others, and *smeared/ghosted* on others. Same input, different output =
  a state/environment bug, not content corruption.
- The eye strips themselves are the game's blink art (extracted from the
  SWFs: 80x35 open / 80x31 half / 80x14 closed), i.e. the *content* is
  fine; the *composite pass* mangles it.

## Root cause

The backend's filter composite (scratch copy + offset copies) issued its
draws **under whatever render state the object's own content draws left
behind**: the current blend mode and colour transform (mult/add). As the
game animates, that leftover state differs per frame, so the same object
composited to a clean or corrupted texture depending on the frame.

## Fix (build `filters16`)

`apply_filter_in_place` now saves `mult_color`/`add_color`, pushes
`BlendMode::Normal` for the internal passes (scratch copy + composite),
and restores the saved state afterwards. Internal diagnostic passes must
never inherit gameplay drawing state.

Also in this build (retained, independently justified):
- scratch texture pooling by size (no per-frame create/delete churn);
- blur filter support (blur <= 24) as weighted offset copies — a
  legibility improvement; it does NOT fix this bug (A/B was
  byte-identical);
- `glFinish()` after each offscreen cache render before any later sample
  of that texture (Mali read-after-write hardening; did not fix this bug
  either, retained as hardening);
- armed stage dumps + entry metadata (all filter variants named).

## Verification protocol (to repeat after any change here)

1. Deploy, relaunch, wait for the game scene.
2. `DREAM_SCRAPE=1` + `DREAM_FILTER_TRACE=1`, catch 30+ blink frames.
3. Check the eye-strip entry's `C-final` is consistent across blinks
   (same content) and that the blink-cycle region states contain no
   smeared-face states; confirm visually via kmsgrab crops.
4. Check `phase_ms_per_render draw=` stays in the usual range (the
   added `glFinish` calls must not regress the frame budget).

## Dead ends (do not re-chase)

- `glFinish` between entry render and composite: fixed nothing visible.
- Scratch pooling: did not change the corruption.
- Blur/filter-type support: unrelated (byte-identical panel A/B).
- Bitmap disk cache: content-addressed (SHA-256 of format+size+pixels),
  cannot serve wrong content.
- Present path (SDL/KMS): falsified by the exact render/panel match.

---

# Update: filters25 session — mechanism localized further (fix still open)

The blend/colour-state fix above did NOT eliminate the visible blink glitch
(verified with frame bursts: the patch still appears, once per blink, ~5 s).
Further findings:

- The blink frame's eye layer (sub-cache 113-114 x 74-77, `filters=1`,
  `blur=1.24`) **re-renders on every blink** and is then drawn *into the
  body sub-cache* (238x450, inside its mask, local 77.5, 60.4, alpha=1.0).
  So the face region of the body texture receives this layer every blink.
- The layer's source is a 341x339 bitmap-filled shape — the character's
  **face animation frames** (217 of them extracted from the SWFs). The
  mask+fill pair is drawn under a shape mask at local (30,27) scale 1.562.
- The uploaded 341x339 texture is **correct** (verified against the
  extracted art: central-crop mean |diff| = 4.6 at 1:1, ~10 % of pixels
  differ only near JPEG edges; the lazy disk-cache pipeline is innocent).
- Template search: the layer's rendered content matches a region of the
  *face frames* at roughly **0.4-0.65x scale** — while the draw transform
  says 1.562x. The composite is effectively sampling the source at a
  wrong (inverted-ish) scale, producing the "smeared" rectangle.
- The backend's `render_shape`/`render_bitmap` are faithful ports of the
  upstream WebGL renderer (differentially verified against
  ruffle@563854a), so the bug is believed to be in how the sub-cache
  context handles the mask+fill pair, or in the fill matrix path for this
  nested case.
- `render_cache_entry` now neutralizes the stencil/mask state around the
  offscreen pass (save → NoMask → restore). This is independently correct
  (offscreen passes must not inherit gameplay mask state) but is not the
  visible fix either.

## Next step (ready, needs one live-scene capture)

`bm_uv` instrument logs the full 3x3 bitmap-fill matrices in armed frames.
With the character live in the scene (blinks re-rendering the eye layer),
capture the eye-fill's matrix (world_tx~30/ty~27, 341x339) and diff it
against the working room fills to pinpoint the transform divergence.
Launcher env: `DREAM_FILTER_TRACE=1 DREAM_UPLOAD_DUMP=1 DREAM_UPLOAD_MAX=400`.
