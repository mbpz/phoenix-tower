"""Dependency-free tests of actual GLB buffers, independent of Blender/bpy.

python3 -m unittest discover -s tools -p test_riverside_assets.py -v
RIVERSIDE_ASSET_DIR=/tmp/second-build python3 -m unittest discover -s tools \
    -p test_riverside_assets.py -v
"""

import hashlib
import json
import math
import os
from pathlib import Path
import struct
import tempfile
import unittest

ASSET_DIR = Path(os.environ.get("RIVERSIDE_ASSET_DIR", str(
    Path(__file__).resolve().parents[1]/"assets/models/riverside")))
# Intentionally NOT imported from generator: catches accidental contract drift.
EXPECTED = {
    "datiji": (7, 2, 7), "hongzhu4": (1, 4, 1), "liangfang5": (5, 1, 1),
    "louban5": (5, 1, 5), "dougong": (2, 2, 2), "jiangting_roof": (7, 3, 7),
}


def read_glb(path):
    data = path.read_bytes()
    assert len(data) >= 28, "Truncated GLB"
    magic, version, length = struct.unpack_from("<III", data)
    assert (magic, version, length) == (0x46546C67, 2, len(data)), "Invalid GLB header"
    chunks = []
    offset = 12
    while offset < len(data):
        size, kind = struct.unpack_from("<I4s", data, offset)
        assert size % 4 == 0 and offset+8+size <= len(data), "Invalid chunk alignment/length"
        chunks.append((kind, data[offset+8:offset+8+size]))
        offset += 8+size
    assert [c[0] for c in chunks] == [b"JSON", b"BIN\0"], "Expected self-contained JSON/BIN GLB"
    document = json.loads(chunks[0][1])
    binary = chunks[1][1]
    assert document["asset"]["version"] == "2.0"
    assert len(document["buffers"]) == 1
    assert "uri" not in document["buffers"][0]
    assert document["buffers"][0]["byteLength"] <= len(binary)
    return document, binary


def accessor_values(document, binary, index):
    accessor = document["accessors"][index]
    assert "sparse" not in accessor, "Sparse accessors are outside this export contract"
    view = document["bufferViews"][accessor["bufferView"]]
    assert view["buffer"] == 0
    formats = {5120: ("b", 1), 5121: ("B", 1), 5122: ("h", 2),
               5123: ("H", 2), 5125: ("I", 4), 5126: ("f", 4)}
    components = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4}
    fmt, size = formats[accessor["componentType"]]
    count = components[accessor["type"]]
    stride = view.get("byteStride", size*count)
    relative = accessor.get("byteOffset", 0)
    start = view.get("byteOffset", 0) + relative
    assert start % size == 0 and stride >= size*count
    assert relative + (accessor["count"]-1)*stride + size*count <= view["byteLength"]
    assert view.get("byteOffset", 0)+view["byteLength"] <= len(binary)
    values = [struct.unpack_from("<"+fmt*count, binary, start+i*stride)
              for i in range(accessor["count"])]
    if accessor.get("normalized", False):
        assert accessor["componentType"] in [5120, 5121, 5122, 5123]
        maximum = {5120: 127, 5121: 255, 5122: 32767, 5123: 65535}[accessor["componentType"]]
        values = [tuple(max(-1, v/maximum) for v in row) for row in values]
    return values


def validate_asset(path, dimensions):
    document, binary = read_glb(path)
    assert len(document["meshes"]) == 1, "Must load exactly Mesh0"
    primitives = document["meshes"][0]["primitives"]
    assert len(primitives) == 1, "All visible geometry must be Primitive0"
    primitive = primitives[0]
    assert primitive.get("mode", 4) == 4, "Must export TRIANGLES"
    assert not primitive.get("extensions") and "targets" not in primitive
    assert len(document["nodes"]) == 1 and document["nodes"][0]["mesh"] == 0
    node = document["nodes"][0]
    assert node.get("translation", [0, 0, 0]) == [0, 0, 0], "Node translation would be lost"
    assert node.get("rotation", [0, 0, 0, 1]) == [0, 0, 0, 1], "Node rotation would be lost"
    assert node.get("scale", [1, 1, 1]) == [1, 1, 1], "Node scale would be lost"
    assert node.get("matrix", [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]) == [
        1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]
    assert document["scenes"][document.get("scene", 0)]["nodes"] == [0]
    assert not any(document.get(k) for k in ["textures", "images", "animations", "skins", "extensionsRequired"])
    attrs = primitive["attributes"]
    assert {"POSITION", "NORMAL", "COLOR_0"} <= attrs.keys(), "Vertex colors are REQUIRED with engine white material"
    positions = accessor_values(document, binary, attrs["POSITION"])
    normals = accessor_values(document, binary, attrs["NORMAL"])
    colors = accessor_values(document, binary, attrs["COLOR_0"])
    position_accessor = document["accessors"][attrs["POSITION"]]
    assert position_accessor["type"] == "VEC3" and position_accessor["componentType"] == 5126
    assert document["accessors"][attrs["COLOR_0"]]["type"] == "VEC4"
    assert document["accessors"][attrs["NORMAL"]]["type"] == "VEC3"
    assert len(positions) == len(normals) == len(colors) > 0
    assert all(math.isfinite(v) for rows in [positions, normals, colors] for row in rows for v in row)
    low = [min(v[i] for v in positions) for i in range(3)]
    high = [max(v[i] for v in positions) for i in range(3)]
    for axis, size in enumerate(dimensions):
        assert abs(low[axis]+size/2) < 1e-5, f"Actual primitive min {low}, expected centered dimensions {dimensions}"
        assert abs(high[axis]-size/2) < 1e-5, f"Actual primitive max {high}, expected centered dimensions {dimensions}"
        assert abs(low[axis]-position_accessor["min"][axis]) < 1e-6
        assert abs(high[axis]-position_accessor["max"][axis]) < 1e-6
    assert all(0 <= v <= 1 for rgba in colors for v in rgba)
    assert all(abs(rgba[3]-1) < 1e-6 for rgba in colors), "All geometry must be opaque"
    assert len(set(colors)) >= 3, "Color cannot depend on material slots/textures"
    assert all(abs(sum(v*v for v in normal)-1) < 2e-4 for normal in normals)
    indices = [v[0] for v in accessor_values(document, binary, primitive["indices"])]
    assert len(indices) % 3 == 0 and 0 < len(indices)//3 < 12_000
    assert all(0 <= index < len(positions) for index in indices)
    assert set(indices) == set(range(len(positions))), "No orphan exported vertices"
    for i in range(0, len(indices), 3):
        a, b, c = (positions[index] for index in indices[i:i+3])
        ab, ac = [b[j]-a[j] for j in range(3)], [c[j]-a[j] for j in range(3)]
        cross = [ab[1]*ac[2]-ab[2]*ac[1], ab[2]*ac[0]-ab[0]*ac[2], ab[0]*ac[1]-ab[1]*ac[0]]
        assert sum(v*v for v in cross) > 1e-16, "Zero-area triangle"
        assert sum(cross[j]*normals[indices[i]][j] for j in range(3)) > 0, "Normal/winding mismatch"
    material = document["materials"][primitive["material"]]
    assert len(document["materials"]) == 1
    assert material["pbrMetallicRoughness"]["baseColorFactor"] == [1, 1, 1, 1]
    assert material.get("alphaMode", "OPAQUE") == "OPAQUE"
    return {"bounds": {"min": low, "max": high}, "vertices": len(positions),
            "triangles": len(indices)//3, "colors_linear_rgba": [list(c) for c in sorted(set(colors))]}


class RiversideAssetsTests(unittest.TestCase):
    def test_all_six_actual_primitive_contracts(self):
        self.assertEqual({p.stem for p in ASSET_DIR.glob("*.glb")}, set(EXPECTED))
        for name, dimensions in EXPECTED.items():
            with self.subTest(asset=name):
                validate_asset(ASSET_DIR/(name+".glb"), dimensions)

    def test_manifest_matches_bytes_not_estimates(self):
        manifest = json.loads((ASSET_DIR/"manifest.json").read_text())
        self.assertEqual(manifest["schema_version"], 1)
        self.assertTrue(manifest["blender_version"])
        self.assertTrue(manifest["generator_version"])
        generator = Path(__file__).with_name("build_riverside_assets.py")
        self.assertEqual(manifest["generator_sha256"], hashlib.sha256(generator.read_bytes()).hexdigest())
        self.assertEqual({r["file"] for r in manifest["assets"]}, {name+".glb" for name in EXPECTED})
        total = 0
        for record in manifest["assets"]:
            with self.subTest(asset=record["file"]):
                path = ASSET_DIR/record["file"]
                self.assertEqual(tuple(record["dimensions"]), EXPECTED[path.stem])
                measured = validate_asset(path, record["dimensions"])
                for key, value in measured.items():
                    self.assertEqual(record[key], value, key)
                data = path.read_bytes()
                self.assertEqual(record["bytes"], len(data))
                self.assertEqual(record["sha256"], hashlib.sha256(data).hexdigest())
                self.assertEqual((record["meshes"], record["primitives"], record["color_attribute"]), (1, 1, "COLOR_0"))
                total += len(data)
        self.assertEqual(manifest["total_glb_bytes"], total)
        self.assertLess(total, 3_000_000)

    def test_editable_source_and_no_external_payloads_or_backups(self):
        self.assertGreater((ASSET_DIR/"riverside.blend").stat().st_size, 10_000)
        self.assertEqual({p.name for p in ASSET_DIR.iterdir()},
                         {"riverside.blend", "manifest.json"} | {name+".glb" for name in EXPECTED})

    def test_validator_rejects_broken_export_contracts(self):
        source, binary = read_glb(ASSET_DIR/"hongzhu4.glb")
        with tempfile.TemporaryDirectory(prefix="riverside-contract-") as directory:
            for defect in ["rotation", "translation", "scale", "color", "primitive", "mesh", "bounds", "actual_bounds"]:
                with self.subTest(defect=defect):
                    document = json.loads(json.dumps(source))
                    payload = binary
                    if defect == "rotation":
                        document["nodes"][0]["rotation"] = [.707, 0, 0, .707]
                    elif defect == "translation":
                        document["nodes"][0]["translation"] = [0, 2, 0]
                    elif defect == "scale":
                        document["nodes"][0]["scale"] = [2, 1, 1]
                    elif defect == "color":
                        del document["meshes"][0]["primitives"][0]["attributes"]["COLOR_0"]
                    elif defect == "primitive":
                        document["meshes"][0]["primitives"] *= 2
                    elif defect == "mesh":
                        document["meshes"] *= 2
                    elif defect == "bounds":
                        document["accessors"][0]["max"][1] = 4
                    elif defect == "actual_bounds":
                        # Keep accessor min/max unchanged: verify actual buffer data,
                        # not just the exporter-provided bounding-box metadata.
                        data = bytearray(binary)
                        struct.pack_into("<f", data, 0, 20.0)
                        payload = bytes(data)
                    encoded = json.dumps(document).encode()
                    encoded += b" " * (-len(encoded) % 4)
                    glb = struct.pack("<III", 0x46546C67, 2, 28+len(encoded)+len(payload))
                    glb += struct.pack("<I4s", len(encoded), b"JSON") + encoded
                    glb += struct.pack("<I4s", len(payload), b"BIN\0") + payload
                    path = Path(directory)/"broken.glb"
                    path.write_bytes(glb)
                    with self.assertRaises(AssertionError):
                        validate_asset(path, EXPECTED["hongzhu4"])

    def test_roof_has_horizontal_ridge_and_swept_hip_corners(self):
        document, binary = read_glb(ASSET_DIR/"jiangting_roof.glb")
        primitive = document["meshes"][0]["primitives"][0]
        positions = accessor_values(document, binary, primitive["attributes"]["POSITION"])
        colors = accessor_values(document, binary, primitive["attributes"]["COLOR_0"])
        normals = accessor_values(document, binary, primitive["attributes"]["NORMAL"])
        # Jade/teal (unlike jade_light ribs) exclusively identify the top skin.
        # Collapsed ridge endpoints must not leave backward-facing tile triangles.
        for color, normal in zip(colors, normals):
            if abs(color[0]-.026) < 1e-6 or abs(color[0]-.021) < 1e-6:
                self.assertGreater(normal[1], 0)
        # A point/cone roof fails: elevated geometry must span a horizontal ridge.
        ridge = [p for p in positions if p[1] > .9 and abs(p[2]) < .25]
        self.assertTrue(ridge)
        self.assertGreater(max(p[0] for p in ridge)-min(p[0] for p in ridge), 2)
        # Both signs on both axes: all four corner upturns must be present.
        for sx in [-1, 1]:
            for sz in [-1, 1]:
                corners = [p[1] for p in positions if p[0]*sx > 3.0 and p[2]*sz > 3.0]
                centers = [p[1] for p in positions if abs(p[0]) < .3 and p[2]*sz > 3.0]
                self.assertTrue(corners and centers)
                self.assertGreater(max(corners), max(centers)+.3)


if __name__ == "__main__":
    unittest.main()
