#!/bin/zsh
# fix_region.sh <in.png> <out.png> <crop x> <crop y> <crop size> <ellipse cx> <ellipse cy> <rx> <ry> <instruction> [run dir]
# Repairs one defect without touching the rest of the picture:
#   1. a square crop (size x size at x,y) goes to xAI images/edits with the instruction;
#   2. the result is scaled back to the crop size;
#   3. only a feathered ellipse (centre and radii in picture coordinates) is pasted back.
# Everything outside the ellipse stays pixel-identical. Request, response and the
# edited crop are kept in the run dir. Key: $XAI_API_KEY, else Keychain api_key_xai.
set -eu
IN=${1:A}; OUT=${2:A}; X=$3; Y=$4; N=$5; CX=$6; CY=$7; RX=$8; RY=$9; INSTR=${10}
RUN=${11:-${OUT:h}/fix_$(date +%H%M%S)}
mkdir -p $RUN
XAI=${XAI_API_KEY:-$(security find-generic-password -a api_key_xai -w 2>/dev/null)}
[[ -n $XAI ]] || { echo "no xAI key"; exit 1; }
magick $IN -crop ${N}x${N}+${X}+${Y} +repage $RUN/crop.png
print -r -- $INSTR > $RUN/instruction.txt
python3 -I - $RUN/crop.png $RUN/instruction.txt $RUN/request.json <<'PY'
import base64, json, sys
img = base64.b64encode(open(sys.argv[1], "rb").read()).decode()
d = {"model": "grok-imagine-image-quality", "prompt": open(sys.argv[2]).read().strip(),
     "image": {"url": "data:image/png;base64," + img}, "n": 1, "response_format": "b64_json"}
open(sys.argv[3], "w").write(json.dumps(d))
PY
code=$(curl -s -m 900 -X POST https://api.x.ai/v1/images/edits -H 'Content-Type: application/json' \
  -H "Authorization: Bearer $XAI" --data-binary @$RUN/request.json -o $RUN/response.json -w "%{http_code}")
echo "images/edits: http $code"
[[ $code == 200 ]] || { head -c 600 $RUN/response.json; echo; exit 1; }
python3 -I -c 'import base64,json,sys; d=json.load(open(sys.argv[1])); open(sys.argv[2],"wb").write(base64.b64decode(d["data"][0]["b64_json"]))' \
  $RUN/response.json $RUN/edited_raw.png
magick $RUN/edited_raw.png -resize ${N}x${N}! $RUN/edited.png
# feathered ellipse mask in crop coordinates
EX=$((CX - X)); EY=$((CY - Y))
# pure greyscale, no alpha: a mask with an alpha channel makes -composite paste the whole crop
magick -size ${N}x${N} xc:black -fill white -draw "ellipse $EX,$EY $RX,$RY 0,360" -blur 0x3 \
  -alpha off -colorspace Gray -type Grayscale $RUN/mask.png
magick $RUN/crop.png -alpha off $RUN/edited.png -alpha off $RUN/mask.png -composite $RUN/patched.png
magick $IN $RUN/patched.png -geometry +${X}+${Y} -composite $OUT
echo "patched: $OUT (run: $RUN)"
