# JPEG Fixtures

These deterministic images exercise JPEG decoding, luma conversion, and VMAF scoring at multiple resolutions. The reference images use FFmpeg's `testsrc2`; distorted images apply a Gaussian blur and stronger JPEG compression.

They were generated with:

```bash
for size in 96x64 160x90 320x180; do
  ffmpeg -f lavfi -i "testsrc2=size=${size}:rate=1" \
    -frames:v 1 -q:v 2 "${size}_reference.jpg"
  ffmpeg -f lavfi -i "testsrc2=size=${size}:rate=1" \
    -vf "gblur=sigma=2.0" -frames:v 1 -q:v 18 \
    "${size}_distorted.jpg"
done
```