"""Pixel measurements shared by the window chrome gates: edges, fills, corners and ink heights in screenshots.

Every function takes a screenshot as an integer array of shape (height, width, 3) and returns numbers in device pixels.
Colour tolerances and expected values belong to the callers; nothing here knows the program under test.
"""

import math

import numpy as np


def grey(a, x, y):
    return int(round(a[int(y), int(x)].mean()))


def blend(a, b, t):
    return a + (b - a) * t


def interior_mode(a, box, inset):
    x, y, w, h = (int(round(v)) for v in box)
    inner = (
        a[y + inset : y + h - inset, x + inset : x + w - inset]
        .mean(axis=2)
        .round()
        .astype(int)
    )
    if inner.size == 0:
        return None
    values, counts = np.unique(inner, return_counts=True)
    return int(values[counts.argmax()])


def cdist(a, b):
    return max(abs(int(x) - int(y)) for x, y in zip(a, b))


def mode_colour(a, y, x0, x1):
    import numpy as np

    keys, counts = np.unique(a[y, x0:x1].reshape(-1, 3), axis=0, return_counts=True)
    return tuple(int(v) for v in keys[counts.argmax()])


def ink_clusters(a, box, bg, gap=3):
    """Column clusters (x0, x1) inside box holding pixels farther than 25 % of the largest distance from bg."""
    import numpy as np

    x0, y0, x1, y1 = box
    d = np.abs(a[y0 : y1 + 1, x0 : x1 + 1] - np.array(bg)).max(axis=2)
    if d.max() < 12:
        return []
    cols = np.where((d >= 0.25 * d.max()).any(axis=0))[0]
    out = []
    for c in cols:
        if out and c - out[-1][1] <= gap:
            out[-1][1] = c
        else:
            out.append([c, c])
    return [(x0 + c0, x0 + c1) for c0, c1 in out]


def ink_height(a, box, bg):
    """Height of the flat-topped glyphs (H, I, 1) in box, edge rows counted by their coverage. Only columns whose
    ink starts and ends on the commonest rows take part (the stems), with neighbours that stay inside those rows,
    so a "~/" or a slash beside the glyphs is left out."""
    span = ink_span(a, box, bg)
    return None if span is None else span[1]


def ink_span(a, box, bg):
    """(top edge, height) of the flat-topped glyphs in box, as ink_height measures them; the top edge is in window
    rows, its partly covered row counted by its coverage."""
    import numpy as np
    from collections import Counter

    x0, y0, x1, y1 = box
    d = np.abs(a[y0 : y1 + 1, x0 : x1 + 1] - np.array(bg)).max(axis=2).astype(float)
    if d.max() < 12 or d.shape[1] < 2:
        return None
    cov = d / d.max()
    ink = cov >= 0.25
    extent = {
        c: (int(np.where(ink[:, c])[0].min()), int(np.where(ink[:, c])[0].max()))
        for c in range(ink.shape[1])
        if ink[:, c].sum() >= 3
    }
    if not extent:
        return None
    top, bottom = Counter(extent.values()).most_common(1)[0][0]
    stems = {c for c, e in extent.items() if e == (top, bottom)}
    near = {c for c in range(ink.shape[1]) if c - 1 in stems or c + 1 in stems}
    inside = {
        c
        for c in near
        if c not in extent or (extent[c][0] >= top - 1 and extent[c][1] <= bottom + 1)
    }
    keep = sorted(stems | inside)
    mask = np.zeros_like(cov)
    mask[max(0, top - 1) : bottom + 2, keep] = 1
    cov = cov * mask
    v = (cov[:, :-1] + cov[:, 1:]).max(axis=1)
    rows = np.where(v > 0.03 * v.max())[0]
    first, last = int(rows.min()), int(rows.max())
    # Full rows: the middle half of the glyphs, where only stems (and at most a crossbar) cross.
    quarter = (last - first) // 4
    full = np.median(v[first + quarter : last - quarter + 1])
    cover = np.minimum(1, v[first : last + 1] / full)
    return y0 + first + 1 - float(cover[0]), float(cover.sum())


def first_stem_box(a, box, bg):
    """The columns around the first glyph's stem in box: the first column with ink down at least three rows, one
    column before it and two after (a stem at 10 to 13 px covers two or three columns). None without ink."""
    import numpy as np

    x0, y0, x1, y1 = box
    d = np.abs(a[y0 : y1 + 1, x0 : x1 + 1] - np.array(bg)).max(axis=2)
    if d.max() < 12:
        return None
    cols = np.where((d >= 0.25 * d.max()).sum(axis=0) >= 3)[0]
    if not len(cols):
        return None
    c = x0 + int(cols[0])
    return (c - 1, y0, c + 2, y1)


def walk(a, x, y, dx, dy, colour, tol=3, limit=2000):
    """Steps from (x, y) by (dx, dy) while the pixel stays within tol of colour; the first pixel that does not."""
    h, w = a.shape[:2]
    for _ in range(limit):
        x, y = x + dx, y + dy
        if not (0 <= x < w and 0 <= y < h) or cdist(a[y, x], colour) > tol:
            return x, y
    return x, y


def corner_area_radius(a, box, fill, base, expected, edge=None, tolerance=3):
    """The radius of a filled rounded rectangle's top-left corner, from the area it leaves out of a square at the
    corner: a circle's corner leaves r^2 (1 - pi/4). With `edge` (the grey of the control's border over its fill),
    the corner's outermost pixels are the border's, so their coverage is read against the border's grey; pixels of
    the fill (within `tolerance`) or anything at least as far from the ground as the border (a knob) count whole."""
    x, y = int(box[0]), int(box[1])
    n = int(math.ceil(expected)) + 3
    sq = a[y : y + n, x : x + n].mean(axis=2)
    if edge is None or abs(edge - fill) <= tolerance:
        frac = np.clip((sq - base) / (fill - base), 0, 1)
    else:
        frac = np.clip((sq - base) / (edge - base), 0, 1)
        whole = (np.abs(sq - fill) <= tolerance) | ((sq - base) / (edge - base) >= 1)
        frac[whole] = 1
    return math.sqrt(max(0.0, (1 - frac).sum()) / (1 - math.pi / 4))


def fill_height(a, box, fill, radius, edge=None, tolerance=3):
    """The height of the fill, in the column clear of the corners where it spans most: from its first row of the fill
    colour (or of the border over it, edge) to its last, so an icon or a label inside it does not shorten it."""
    x, y, w, h = (int(round(v)) for v in box)
    lo, hi = y - 3, y + h + 3
    best = 0
    for cx in range(x + int(math.ceil(radius)) + 1, x + w - int(math.ceil(radius)) - 1):
        col = a[lo:hi, cx].mean(axis=1)
        match = np.abs(col - fill) <= tolerance
        if edge is not None:
            match |= np.abs(col - edge) <= tolerance
        rows = np.where(match)[0]
        if len(rows):
            best = max(best, int(rows[-1] - rows[0] + 1))
    return best


def circle_fit_radius(cov, left, top, row, col, reach, max_radius):
    """Radius of a top-left corner whose straight edges are at column left and row top: per row and column near the
    corner, the uncovered run from outside to the edge, fitted with a quarter circle (as pane-radius does)."""
    scan_x, scan_y = left - 3, top - 3

    def run_x(r):
        n, total = scan_x, 0.0
        while n < scan_x + reach and cov[r, n] < 0.98:
            total += 1 - cov[r, n]
            n += 1
        return scan_x + total if n < scan_x + reach else None

    def run_y(c):
        n, total = scan_y, 0.0
        while n < scan_y + reach and cov[n, c] < 0.98:
            total += 1 - cov[n, c]
            n += 1
        return scan_y + total if n < scan_y + reach else None

    edge_x, edge_y = run_x(row), run_y(col)
    if edge_x is None or edge_y is None:
        raise ValueError("the corner edges were not found")
    points = [(run_x(r), r + 0.5, "x") for r in range(scan_y, scan_y + reach)]
    points += [(c + 0.5, run_y(c), "y") for c in range(scan_x, scan_x + reach)]
    points = [q for q in points if q[0] is not None and q[1] is not None]

    def model(r, q):
        if q[2] == "x":
            if q[1] < edge_y:
                return None
            dy = edge_y + r - q[1]
            return q[0] - (
                edge_x if dy <= 0 else edge_x + r - math.sqrt(max(0.0, r * r - dy * dy))
            )
        if q[0] < edge_x:
            return None
        dx = edge_x + r - q[0]
        return q[1] - (
            edge_y if dx <= 0 else edge_y + r - math.sqrt(max(0.0, r * r - dx * dx))
        )

    def cost(r):
        res = [m for m in (model(r, q) for q in points) if m is not None]
        return sum(m * m for m in res) / max(1, len(res))

    return min((i / 20 for i in range(0, int(20 * 2 * max_radius) + 1)), key=cost)


def coverage_height(a, box, bg):
    """The height of a flat-topped, flat-bottomed glyph (a digit): per row the strongest sum of two neighbouring
    pixels (a stem split across two columns still counts whole), as a share of the same sum in the glyph's full rows
    (the median of its middle half), capped at 1; summed over the rows, so partly covered top and bottom rows count
    their coverage."""
    x0, y0, x1, y1 = box
    d = np.abs(a[y0:y1, x0:x1] - np.array(bg)).max(axis=2).astype(float)
    if d.max() < 12 or d.shape[1] < 2:
        return None
    cov = d / d.max()
    v = (cov[:, :-1] + cov[:, 1:]).max(axis=1)
    rows = np.where(v > 0.03 * v.max())[0]
    first, last = int(rows.min()), int(rows.max())
    quarter = (last - first) // 4
    full = np.median(v[first + quarter : last - quarter + 1])
    return float(np.minimum(1, v[first : last + 1] / full).sum())
