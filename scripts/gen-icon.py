"""生成 DataDock 应用图标源图（1024x1024 PNG）。

仅使用标准库（zlib + struct），避免引入图像处理依赖。
图形：深色圆角方形底 + 三根渐变蓝色柱状条，象征数据底座。
"""

import struct
import zlib

SIZE = 1024
SS = 2  # 超采样倍数，用于抗锯齿
W = SIZE * SS


def lerp(a, b, t):
    return a + (b - a) * t


def round_rect_coverage(x, y, cx, cy, hw, hh, r):
    """返回点 (x, y) 是否在圆角矩形内（1/0），坐标为像素中心。"""
    dx = abs(x - cx) - (hw - r)
    dy = abs(y - cy) - (hh - r)
    dx = max(dx, 0.0)
    dy = max(dy, 0.0)
    return 1.0 if (dx * dx + dy * dy) <= r * r else 0.0


def bg_color(t):
    """背景垂直渐变：上浅下深。"""
    top = (42, 52, 66)
    bottom = (18, 24, 32)
    return tuple(int(lerp(top[i], bottom[i], t)) for i in range(3))


def main():
    s = SIZE
    # 预计算几何参数（单位：1024 画布）
    radius = s * 0.225
    bars = [
        # (中心x比例, 高度比例, 颜色)
        (0.345, 0.30, (59, 130, 246)),
        (0.500, 0.46, (96, 165, 250)),
        (0.655, 0.37, (59, 130, 246)),
    ]
    bar_w = s * 0.098
    base_y = 0.735 * s
    bar_radius = bar_w / 2

    rows = []
    for py in range(SS * s):
        y = (py + 0.5) / SS
        row = bytearray()
        t = y / s
        base = bg_color(t)
        for px in range(SS * s):
            x = (px + 0.5) / SS
            inside = round_rect_coverage(x, y, s / 2, s / 2, s / 2, s / 2, radius)
            if inside <= 0.0:
                row += bytes((0, 0, 0, 0))
                continue

            # 背景 + 轻微内侧高光
            color = base
            edge = round_rect_coverage(x, y, s / 2, s / 2, s / 2 - 1.5, s / 2 - 1.5, radius)
            if edge <= 0.0:
                color = tuple(min(255, c + 26) for c in base)

            for cxf, hf, bar_color in bars:
                h = s * hf
                cy = base_y - h / 2
                if round_rect_coverage(x, y, s * cxf, cy, bar_w / 2, h / 2, bar_radius) > 0.0:
                    color = bar_color
                    break

            row += bytes((color[0], color[1], color[2], 255))
        rows.append(row)

    # 下采样（SS x SS 盒式滤波）
    out = bytearray()
    for oy in range(s):
        out.append(0)  # PNG 行过滤类型：None
        for ox in range(s):
            r = g = b = a = 0
            for dy in range(SS):
                src = rows[oy * SS + dy]
                for dx in range(SS):
                    i = (ox * SS + dx) * 4
                    r += src[i]
                    g += src[i + 1]
                    b += src[i + 2]
                    a += src[i + 3]
            n = SS * SS
            out += bytes((r // n, g // n, b // n, a // n))

    def chunk(tag, data):
        return (
            struct.pack(">I", len(data))
            + tag
            + data
            + struct.pack(">I", zlib.crc32(tag + data) & 0xFFFFFFFF)
        )

    ihdr = struct.pack(">IIBBBBB", s, s, 8, 6, 0, 0, 0)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", ihdr)
        + chunk(b"IDAT", zlib.compress(bytes(out), 9))
        + chunk(b"IEND", b"")
    )

    with open("src/assets/icons/app-icon.png", "wb") as f:
        f.write(png)
    print("written src/assets/icons/app-icon.png", len(png), "bytes")


if __name__ == "__main__":
    main()
