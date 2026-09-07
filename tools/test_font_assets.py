"""Check bundled CJK coverage without installing font-processing dependencies."""
import pathlib
import struct
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[1]


def required_characters():
    text = ''.join(p.read_text() for base, ext in [('src', '*.rs'), ('resources', '*.ron')]
                   for p in (ROOT / base).rglob(ext))
    return {ord(c) for c in text if '\u3400' <= c <= '\u9fff'} | set(range(32, 127))


def mapped_characters(data):
    """Read Unicode BMP format-4 cmap, excluding entries mapped to .notdef."""
    u16 = lambda at: struct.unpack_from('>H', data, at)[0]
    for i in range(u16(4)):
        tag, _, offset, _ = struct.unpack_from('>4sIII', data, 12 + i * 16)
        if tag != b'cmap':
            continue
        for j in range(u16(offset + 2)):
            platform, encoding, relative = struct.unpack_from('>HHI', data, offset + 4 + j * 8)
            table = offset + relative
            if platform != 3 or encoding != 1 or u16(table) != 4:
                continue
            count = u16(table + 6) // 2
            end_at = table + 14
            start_at = end_at + count * 2 + 2
            delta_at = start_at + count * 2
            range_at = delta_at + count * 2
            mapped = set()
            for k in range(count):
                start, end = u16(start_at + k * 2), u16(end_at + k * 2)
                delta, relative = u16(delta_at + k * 2), u16(range_at + k * 2)
                for point in range(start, end + 1):
                    glyph = (point + delta) & 0xffff
                    if relative:
                        glyph = u16(range_at + k * 2 + relative + (point - start) * 2)
                        if glyph:
                            glyph = (glyph + delta) & 0xffff
                    if glyph:
                        mapped.add(point)
            return mapped
    raise AssertionError('Bundled font needs a Unicode BMP format-4 cmap')


class FontAssetsTest(unittest.TestCase):
    def test_all_repository_cjk_and_ascii_are_covered(self):
        data = (ROOT / 'assets/fonts/NotoSansSC-subset.otf').read_bytes()
        missing = required_characters() - mapped_characters(data)
        self.assertFalse(missing, 'Missing glyphs: ' + ''.join(chr(c) for c in sorted(missing)))
        self.assertLess(len(data), 2_000_000, 'Keep the runtime font a subset, not a full CJK font')


if __name__ == '__main__':
    unittest.main()
