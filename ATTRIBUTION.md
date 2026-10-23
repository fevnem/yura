# Third-party assets

Yura's source is MIT. The art and voice models it can load are not, and are
deliberately **not committed** to this repository — each has its own license
embedded in the file, and that file is the authority.

## 3D avatars (VRM)

`npm run avatar:fetch` downloads the default character into
`ui/public/assets/models/`. The loader accepts any VRM 0.x / VRM 1.0 file, so
you can drop your own in and select it in Settings → Avatar.

The default download is a **development convenience**, not a distributable
asset: run it on your own machine, and do not ship the downloaded file inside a
build. The installer contains no avatar — on first launch Yura appears in her
procedural spirit form until you give her a body.

| Model | Author | Terms |
|---|---|---|
| AvatarSample_A / B / C | VRoid Project | VRM meta grants `redistribution: allow`, commercial use allowed, credit optional. Verify the embedded meta before redistributing. |
| Vita | VRM Consortium | CC0, all permissions including redistribution of modified works. |
| Your own | You / commissioned | Whatever you decide. |

Models you must **not** ship inside a product: VRoid Hub download-only
characters, and anything with revocable "creator discretion" clauses (e.g.
Alicia Solid) — fine locally, radioactive in a distributed build.

## Voices

Voice is delegated to the WebView2 speech engine (Windows SAPI / OneCore
voices). Nothing is bundled. When the Kokoro backend lands it will fetch its
ONNX weights at runtime, not ship them.

## Fonts

System font stack only — no webfont is redistributed.
