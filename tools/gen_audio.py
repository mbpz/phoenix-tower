#!/usr/bin/env python3
"""程序化生成游戏音效（B-19）。

纯 Python 标准库（wave/math/random），无第三方依赖。
输出 44100Hz 16-bit 单声道 WAV 到 assets/audio/。
"""
import math
import os
import random
import struct
import wave

SR = 44100
OUT = os.path.join(os.path.dirname(__file__), "..", "assets", "audio")


def write_wav(name, samples):
    path = os.path.join(OUT, name)
    with wave.open(path, "w") as w:
        w.setnchannels(1)
        w.setsampwidth(2)
        w.setframerate(SR)
        frames = b"".join(
            struct.pack("<h", max(-32767, min(32767, int(s * 32767))))
            for s in samples
        )
        w.writeframes(frames)
    print(f"  {name}: {len(samples)/SR:.2f}s")


def noise(n, seed=0):
    rng = random.Random(seed)
    return [rng.uniform(-1, 1) for _ in range(n)]


def exp_decay(n, rate):
    return [math.exp(-rate * i / SR) for i in range(n)]


def tone(freq, n, amp=1.0, detune=0.0):
    return [
        amp * math.sin(2 * math.pi * (freq + detune) * i / SR) for i in range(n)
    ]


def make_wood():
    """放置 - 木：低频钝响 + 短噪声，清脆木石碰撞感。"""
    n = int(0.12 * SR)
    nz = noise(n, seed=11)
    body = [x * math.exp(-i / SR * 45) for i, x in enumerate(nz)]
    thump = [0.55 * math.sin(2 * math.pi * 160 * i / SR) * math.exp(-i / SR * 28) for i in range(n)]
    click = [0.5 * math.sin(2 * math.pi * 620 * i / SR) * math.exp(-i / SR * 90) for i in range(n)]
    return [0.45 * a + 0.4 * b + 0.25 * c for a, b, c in zip(body, thump, click)]


def make_stone():
    """放置 - 石/瓦：高频短促点击 + 中频共鸣。"""
    n = int(0.14 * SR)
    nz = noise(n, seed=23)
    body = [x * math.exp(-i / SR * 70) for i, x in enumerate(nz)]
    tick = [0.6 * math.sin(2 * math.pi * 1150 * i / SR) * math.exp(-i / SR * 160) for i in range(n)]
    hum = [0.4 * math.sin(2 * math.pi * 310 * i / SR) * math.exp(-i / SR * 40) for i in range(n)]
    return [0.4 * a + 0.5 * b + 0.35 * c for a, b, c in zip(body, tick, hum)]


def make_bell():
    """完成 - 编钟：C5/G5/C6 泛音和弦，长衰减。"""
    n = int(3.0 * SR)
    decay = exp_decay(n, 1.4)
    parts = [
        tone(523.25, n, 1.0, detune=1.5),   # C5
        tone(783.99, n, 0.6, detune=-1.0),  # G5
        tone(1046.5, n, 0.4, detune=1.2),   # C6
        tone(1568.0, n, 0.18, detune=-0.8), # G6
    ]
    out = []
    for i in range(n):
        s = sum(p[i] for p in parts) * decay[i]
        out.append(s * 0.4)
    return out


def make_river():
    """环境 - 长江水声：低频滤波噪声循环，柔和起伏。"""
    n = int(8.0 * SR)
    rng = random.Random(7)
    prev = 0.0
    out = []
    alpha = 0.02  # 低通：只保留低频"水声"
    slow = 0.0
    for i in range(n):
        w = rng.uniform(-1, 1)
        prev = prev + alpha * (w - prev)  # 一阶低通
        slow = 0.995 * slow + 0.005 * prev
        # 幅度缓慢呼吸
        breath = 0.75 + 0.25 * math.sin(2 * math.pi * 0.08 * i / SR)
        out.append(prev * 0.5 * breath)
    return out


def make_wind_chime():
    """环境 - 风铃：随机时点的清脆铃音。"""
    n = int(6.0 * SR)
    out = [0.0] * n
    rng = random.Random(31)
    freqs = [880.0, 987.8, 1174.7, 1318.5, 1568.0]  # A5 B5 D6 E6 G6
    t = 0.0
    while t < n - 2 * SR:
        f = rng.choice(freqs)
        dur = int(SR * rng.uniform(0.8, 1.6))
        amp = rng.uniform(0.05, 0.16)
        for j in range(min(dur, n - int(t))):
            env = math.exp(-j / SR * 3.0)
            out[int(t) + j] += amp * (math.sin(2 * math.pi * f * j / SR) + 0.3 * math.sin(2 * math.pi * f * 2.77 * j / SR)) * env
        t += SR * rng.uniform(0.6, 1.8)
    return out


if __name__ == "__main__":
    os.makedirs(OUT, exist_ok=True)
    write_wav("place_wood.wav", make_wood())
    write_wav("place_stone.wav", make_stone())
    write_wav("bell_chime.wav", make_bell())
    write_wav("river_loop.wav", make_river())
    write_wav("wind_chime_loop.wav", make_wind_chime())
    print("done")
