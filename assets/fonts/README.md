# Bundled Chinese font

`NotoSansSC-subset.otf` is a 442,980-byte subset of Noto Sans CJK SC Regular,
licensed under the SIL Open Font License 1.1; see `OFL.txt`.
The source includes the existing Noto naming records; this is a subset, not a new typeface.

Source retrieved 2026-09-07:
- Font: https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/OTF/SimplifiedChinese/NotoSansCJKsc-Regular.otf
- License: https://raw.githubusercontent.com/notofonts/noto-cjk/main/Sans/LICENSE
- Source SHA-256: `2c76254f6fc379fddfce0a7e84fb5385bb135d3e399294f6eeb6680d0365b74b`
- Subset SHA-256: `3945befd18dfe455ec6d04e90fecbd94e29c6b70db063f8fe56a20e6eccd745c`

No font-processing package is required to run the game or coverage tests.
To regenerate, obtain and hash-check the source above, then use the existing
HarfBuzz `hb-subset` CLI (used here: the locally installed Homebrew executable):

```bash
python3 - <<'PY'
import sys
from pathlib import Path
sys.path.insert(0, 'tools')
from test_font_assets import required_characters
points = (required_characters() | set(range(0x2000, 0x2070))
          | set(range(0x3000, 0x3040)) | set(range(0xff00, 0xfff0))
          | {0xb7, 0xb0, 0x2190, 0x2192})
Path('/tmp/phoenix-font-unicodes.txt').write_text(','.join(f'{p:X}' for p in sorted(points)))
PY
hb-subset /tmp/phoenix-NotoSansCJKsc-Regular.otf \
  --unicodes-file=/tmp/phoenix-font-unicodes.txt \
  --output-file=assets/fonts/NotoSansSC-subset.otf
python3 -m unittest discover -s tools -p test_font_assets.py
```

The regression reads the actual cmap using Python's standard library and checks
ASCII plus all CJK characters in Rust and RON source, including 亭/庭/院.
It does not promise arbitrary imported Unicode or emoji coverage, or fix the
separate ICU4X missing-segmentation-model diagnostics.
