#!/bin/zsh
# trace.sh <input.png> <out.svg> [glass colours=40] [stone colours=16] [turdsize=6] [lead: brightness<23%, or brightness<66% and saturation<24%, thin only]
# Traces the photograph of the window into a layered SVG:
#   1. 2x upscale and a 3x3 median against JPEG noise;
#   2. the masonry (everything outside the glazed lights) is quantised to a
#      smaller palette and traced first, mouldings, ribs and jambs included;
#   3. the glass (the two lights, the rose and the spandrel lights) is
#      quantised to many colours, each traced as its own layer with potrace;
#   4. the lead, every dark unsaturated pixel of the glass, goes last.
# The glass pass sees the masonry as the flat WALL colour, the stone pass sees
# the glass as WALL; neither wall layer is written, a gradient base shows
# through any gap when the SVG is embedded (src/traced.rs).
set -eu
IN=$1; OUT=$2; NG=${3:-40}; NS=${4:-16}; TURD=${5:-6}; LEAD_T=${6:-23%}; LEAD_T2=${7:-66%}; LEAD_S=${8:-24%}; LEAD_W=${9:-7}; STONE_LEVEL=${10:-40}; LEAD_TURD=${11:-40}
# FLAT=1: a clean flat source (no photograph): no upscaling, a light median
# only, no geometry masks, no stone pass; every colour is traced. Use with a
# MEDIAN=0 skips the median (for a picture already flattened piece by piece
# with tools/flatten). CC_AREA (default 40): after quantising, colour islands under that many
# pixels are merged into their neighbours, which removes the grain of the
# source without blurring faces or hatching.
FLAT=${FLAT:-0}
# seam stroke on each colour layer, hides hairline gaps between traced layers;
# a flattened source has pixel-exact boundaries, so 1 px is enough there
SEAM=${SEAM:-$([[ $FLAT == 1 ]] && echo 1 || echo 3)}
WALL="#1b1a2e"
W=$(dirname "$OUT")/work
rm -rf "$W"; mkdir -p "$W"
if [[ $FLAT == 1 && ${MEDIAN:-1} == 0 ]]; then magick "$IN" "$W/src.png"; elif [[ $FLAT == 1 ]]; then magick "$IN" -statistic Median 3x3 "$W/src.png"; else magick "$IN" -resize 200% -statistic Median 3x3 "$W/src.png"; fi
PW=$(magick identify -format %w "$W/src.png"); PH=$(magick identify -format %h "$W/src.png")

# masks in 2x coordinates: the glazed openings, and the whole wall opening
magick -size ${PW}x${PH} xc:black -fill white \
  -draw "scale 2,2 path 'M 82,1200 L 130,400 A 139.5,139.5 0 0 1 395,400 L 392,1200 Z'" \
  -draw "scale 2,2 path 'M 448,1200 L 445,400 A 139.5,139.5 0 0 1 710,400 L 762,1200 Z'" \
  -draw "scale 2,2 circle 420,182 420,330" \
  -draw "scale 2,2 polygon 170,302 286,180 300,300" \
  -draw "scale 2,2 polygon 670,302 554,180 540,300" \
  -draw "scale 2,2 polygon 180,330 660,330 660,300 180,300" \
  "$W/glass_geo.png"
magick "$W/glass_geo.png" -morphology Dilate Disk:6 "$W/glass.png"
if [[ $FLAT == 1 ]]; then
  magick -size ${PW}x${PH} xc:white "$W/glass_geo.png"; cp "$W/glass_geo.png" "$W/glass.png"
fi
# the masonry: everything that is not glass (the whole wall is traced)
magick "$W/glass.png" -negate "$W/stone.png"

flatten() { # <mask> <out>: the picture where the mask is white, WALL elsewhere
  magick "$W/src.png" \( +clone -fill "$WALL" -colorize 100 \) \( "$1" -alpha off -negate \) -compose Over -composite "$2"
}
palette() { # <quant.png> <palette.txt>
  magick "$1" -format %c histogram:info:- | sed -E 's/^ *([0-9]+):.*(#[0-9A-Fa-f]{6}).*$/\1 \2/' | sort -rn > "$2"
}
layer() { # <quant.png> <hex> <id> <turd> <stroke> [fill hex]: one traced colour as a <g>
  magick "$1" -fill white +opaque "$2" -fill black -opaque "$2" -threshold 50% "$W/$3.pbm"
  layer_mask "$W/$3.pbm" "${6:-$2}" "$3" $4 $5
}
unstretch() { # <hex of the level-stretched stone> -> hex of the real shade
  local r=$((16#${1[2,3]} * STONE_LEVEL / 100)) g=$((16#${1[4,5]} * STONE_LEVEL / 100)) b=$((16#${1[6,7]} * STONE_LEVEL / 100))
  printf '#%02X%02X%02X' $r $g $b
}
layer_mask() { # <mask.pbm, black = shape> <hex> <id> <turd> <stroke>
  potrace "$1" -s -u 10 -t $4 -a 1.0 -O 0.4 -o "$W/$3.svg" 2>/dev/null
  if [[ $5 == 0 ]]; then
    sed -n '/<g /,/<\/g>/p' "$W/$3.svg" | sed -E "s/fill=\"#000000\" stroke=\"none\"/fill=\"$2\"/" | sed -E "1s/<g /<g id=\"$3\" data-color=\"$2\" /"
  else
    sed -n '/<g /,/<\/g>/p' "$W/$3.svg" | sed -E "s/fill=\"#000000\" stroke=\"none\"/fill=\"$2\" stroke=\"$2\" stroke-width=\"$5\" stroke-linejoin=\"round\"/" | sed -E "1s/<g /<g id=\"$3\" data-color=\"$2\" /"
  fi
}
darkest() { # <palette.txt>
  while read cnt hex; do
    r=$((16#${hex[2,3]})); g=$((16#${hex[4,5]})); b=$((16#${hex[6,7]}))
    echo "$(( (r*299+g*587+b*114)/1000 )) $hex"
  done < "$1" | sort -n | head -1 | cut -d' ' -f2
}

if [[ $FLAT == 1 ]]; then : > "$W/palette_stone.txt"; else
flatten "$W/stone.png" "$W/flat_stone.png"
# the masonry lives in the darkest 40 % of the range: stretch it before
# quantising so the mouldings get their own colours, scale the palette back after
magick "$W/flat_stone.png" -blur 0x1.5 -level 0%,$STONE_LEVEL% +dither -quantize LAB -colors $NS -define png:color-type=2 "$W/qs.png"
palette "$W/qs.png" "$W/palette_stone.txt"
fi
flatten "$W/glass.png" "$W/flat_glass.png"
magick "$W/flat_glass.png" +dither -quantize LAB -colors $NG -define png:color-type=2 "$W/qg.png"
if [[ $FLAT == 1 && ${CC_AREA:-40} -gt 0 ]]; then
  # grain: colour islands under CC_AREA px are merged into their neighbours
  magick "$W/qg.png" -define connected-components:area-threshold=${CC_AREA:-40} -define connected-components:mean-color=true -connected-components 8 -define png:color-type=2 "$W/qg.png"
fi
palette "$W/qg.png" "$W/palette_glass.txt"
echo "stone palette:"; cat "$W/palette_stone.txt"
echo "glass palette ($(wc -l < "$W/palette_glass.txt" | tr -d ' ') colours):"; cat "$W/palette_glass.txt"
LEAD=$(darkest "$W/palette_glass.txt")
WALL_S=$(head -1 "$W/palette_stone.txt" | cut -d' ' -f2)
WALL_G=$(head -1 "$W/palette_glass.txt" | cut -d' ' -f2)
[[ $FLAT == 1 ]] && WALL_G=none   # nothing is wall: every colour is traced

{
  echo "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"$PW\" height=\"$PH\" viewBox=\"0 0 $PW $PH\" data-wall=\"$WALL_G\">"
  i=0
  while read cnt hex; do
    [[ $hex == $WALL_S ]] && continue
    i=$((i+1)); layer "$W/qs.png" "$hex" "s$i" 60 4 "$(unstretch $hex)"
  done < "$W/palette_stone.txt"
  i=0
  while read cnt hex; do
    [[ $hex == $WALL_G ]] && continue
    i=$((i+1)); layer "$W/qg.png" "$hex" "g$i" $TURD $SEAM
  done < "$W/palette_glass.txt"
  # the lead: every dark pixel of the glass (strongest channel below LEAD_T),
  # and every dark grey one (below LEAD_T2 with saturation below LEAD_S: the
  # leads in the pale quarries), from the filtered picture itself so the
  # lines stay continuous; only structures thinner than 2*LEAD_W px count,
  # so stone, saddle bars and lions stay with their traced colours;
  # traced without dropping fragments, drawn last
  magick "$W/src.png" -colorspace HSB -channel B -separate +channel -threshold $LEAD_T -negate "$W/lead1.png"
  magick "$W/src.png" -colorspace HSB -channel B -separate +channel -threshold $LEAD_T2 -negate "$W/lead_b.png"
  magick "$W/src.png" -colorspace HSB -channel S -separate +channel -threshold $LEAD_S -negate "$W/lead_s.png"
  magick "$W/lead_b.png" "$W/lead_s.png" -compose Multiply -composite "$W/lead2.png"
  magick "$W/lead1.png" "$W/lead2.png" -compose Lighten -composite \( "$W/glass_geo.png" -alpha off \) -compose Multiply -composite -threshold 50% "$W/dark.png"
  if [[ $FLAT == 1 ]]; then
    # a flattened source already has its outlines decided and written as pure
    # black: the lead is exactly those pixels, nothing grey, no thinness test
    magick "$W/src.png" -colorspace HSB -channel B -separate +channel -threshold 8% "$W/lead.pbm"
  else
  # only thin structures are lead: an opening removes everything wider than a came
  magick "$W/dark.png" -morphology Open Disk:$LEAD_W "$W/thick.png"
  magick "$W/dark.png" \( "$W/thick.png" -negate \) -compose Multiply -composite -negate -threshold 50% "$W/lead.pbm"
  fi
  # fragments under LEAD_TURD px² are the mottling of the glass, not came
  layer_mask "$W/lead.pbm" "$LEAD" lead $LEAD_TURD 0
  echo "</svg>"
} > "$OUT"
echo "lead $LEAD, wall $WALL_G (stone pass $WALL_S)"
echo "wrote $OUT ($(wc -c < "$OUT" | tr -d ' ') bytes)"
