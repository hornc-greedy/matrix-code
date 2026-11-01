"""
Builds assets/matrix-code.ttf from Rezmason's Matrix-Code.ttf. Its glyphs are
assigned to the characters from U+100000 on in the Supplementary Private Use
Area-B, which other fonts leave empty, so terminals fall back to this font for
them.

Usage: uv run --with fonttools python tools/build_font.py Matrix-Code.ttf
"""

import pathlib
import sys

from fontTools import subset
from fontTools.pens.recordingPen import DecomposingRecordingPen
from fontTools.pens.transformPen import TransformPen
from fontTools.pens.ttGlyphPen import TTGlyphPen
from fontTools.ttLib import TTFont
from fontTools.ttLib.tables._c_m_a_p import CmapSubtable

FIRST_CHARACTER = 0x100000
FAMILY = "matrix-code"
# The program leaves the cell right of each character empty, so the glyphs
# grow into it and stay centered on the row
SCALE = 1.2
SHIFT = -0.2
OUTPUT = pathlib.Path(__file__).parent.parent / "assets" / "matrix-code.ttf"

# Keeping the original's timestamp makes every build byte for byte the same
font = TTFont(sys.argv[1], recalcTimestamp=False)
glyf = font["glyf"]
hmtx = font["hmtx"]
glyph_set = font.getGlyphSet()

# Control characters, the space and U+FFFD map to empty outlines
names = []
for _, name in sorted(font.getBestCmap().items()):
    glyph = glyf[name]
    if glyph.numberOfContours and glyph.xMax > glyph.xMin:
        names.append(name)
mapping = {FIRST_CHARACTER + index: name for index, name in enumerate(names)}

recordings = {}
for name in names:
    recordings[name] = DecomposingRecordingPen(glyph_set)
    glyph_set[name].draw(recordings[name])
shift = SHIFT * font["head"].unitsPerEm
for name in names:
    pen = TTGlyphPen(None)
    recordings[name].replay(TransformPen(pen, (SCALE, 0, 0, SCALE, 0, shift)))
    glyph = pen.glyph()
    glyph.recalcBounds(glyf)
    glyf[name] = glyph
    hmtx[name] = (hmtx[name][0], glyph.xMin)

# Format 12 holds code points beyond U+FFFF
tables = []
for platform_id, encoding_id in ((0, 4), (3, 10)):
    table = CmapSubtable.newSubtable(12)
    table.platformID = platform_id
    table.platEncID = encoding_id
    table.language = 0
    table.cmap = dict(mapping)
    tables.append(table)
font["cmap"].tables = tables

name_table = font["name"]
name_table.names = []
for name_id, value in {
    0: "Copyright (c) 2018 Rezmason, MIT License",
    1: FAMILY,
    2: "Regular",
    3: FAMILY,
    4: FAMILY,
    6: FAMILY,
}.items():
    name_table.setName(value, name_id, 3, 1, 0x409)
    name_table.setName(value, name_id, 1, 0, 0)

options = subset.Options()
options.name_IDs = ["*"]
options.name_languages = ["*"]
options.layout_features = []
subsetter = subset.Subsetter(options)
subsetter.populate(unicodes=mapping)
subsetter.subset(font)
font["OS/2"].recalcUnicodeRanges(font)

font.save(OUTPUT)
print(f"wrote {len(mapping)} glyphs to {OUTPUT}")
