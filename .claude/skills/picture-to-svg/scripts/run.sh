#!/bin/zsh
# run.sh <original.png> <instruction.txt> <out dir>
# The whole conversion after the picture exists:
#   1. flat source: the original + instruction to xAI images/edits (skipped if
#      <out>/edit/image.png exists, so a good edit is never thrown away)
#   2. 2x upscale (Mitchell)
#   3. flatten: outlines, then colour-grown pieces (scripts/flatten, Rust)
#   4. trace: one potrace layer per colour, lead last (scripts/trace.sh, FLAT mode)
# Output: <out>/edit/{request.json,response.json,image.png}, <out>/edit2x.png,
#         <out>/flat2x.png, <out>/traced.svg
# Key: $XAI_API_KEY, else the Keychain item api_key_xai.
set -eu
IN=${1:A}; INSTR=${2:A}; OUT=${3:A}
HERE=${0:A:h}
mkdir -p $OUT/edit
echo "disk: $(df -h / | tail -1 | awk '{print $4}') free (a 2x trace needs about 200 MB of work files)"

# 1. flat source
if [[ ! -s $OUT/edit/image.png ]]; then
  XAI=${XAI_API_KEY:-$(security find-generic-password -a api_key_xai -w 2>/dev/null)}
  [[ -n $XAI ]] || { echo "no xAI key"; exit 1; }
  cp $INSTR $OUT/edit/instruction.txt
  python3 -I - $IN $INSTR $OUT/edit/request.json <<'PY'
import base64, json, sys
img = base64.b64encode(open(sys.argv[1], "rb").read()).decode()
d = {"model": "grok-imagine-image-quality", "prompt": open(sys.argv[2]).read().strip(),
     "image": {"url": "data:image/png;base64," + img}, "n": 1, "response_format": "b64_json",
     "resolution": "2k"}
open(sys.argv[3], "w").write(json.dumps(d))
PY
  code=$(curl -s -m 900 -X POST https://api.x.ai/v1/images/edits -H 'Content-Type: application/json' \
    -H "Authorization: Bearer $XAI" --data-binary @$OUT/edit/request.json -o $OUT/edit/response.json -w "%{http_code}")
  echo "images/edits: http $code"
  [[ $code == 200 ]] || { head -c 600 $OUT/edit/response.json; echo; exit 1; }
  python3 -I -c 'import base64,json,sys; d=json.load(open(sys.argv[1])); open(sys.argv[2],"wb").write(base64.b64decode(d["data"][0]["b64_json"]))' \
    $OUT/edit/response.json $OUT/edit/image.png
  echo "flat source: $OUT/edit/image.png  (look at it before going on; rerun keeps it)"
fi

# 2. upscale
magick $OUT/edit/image.png -filter Mitchell -resize 200% $OUT/edit2x.png

# 3. flatten
FLATTEN=${FLATTEN:-}
if [[ -z $FLATTEN ]]; then
  (cd $HERE/flatten && cargo build --release --offline -q)
  TD=$(cd $HERE/flatten && cargo metadata --format-version 1 --no-deps 2>/dev/null | python3 -I -c 'import json,sys; print(json.load(sys.stdin)["target_directory"])')
  FLATTEN=$TD/release/flatten
fi
$FLATTEN $OUT/edit2x.png $OUT/flat2x.png 0.42 48 8 80 24 32

# 4. trace
FLAT=1 MEDIAN=0 CC_AREA=0 zsh $HERE/trace.sh $OUT/flat2x.png $OUT/traced.svg 96 16 16 23% 66% 24% 7 40 32
echo "done: $OUT/traced.svg"
