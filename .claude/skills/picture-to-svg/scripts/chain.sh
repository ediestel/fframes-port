#!/bin/zsh
# Chain: a model writes the image prompt for chain_query.md, the prompt goes
# unchanged to the xAI image API, fframes renders the picture from the image.
#
#   chain.sh mode_s   prompt from the NTGT engine /evaluate, mode_s, xai, top@low
#   chain.sh direct   prompt from grok-4.7 alone, straight to the xAI API
#   PROMPT_FILE=f chain.sh <name>   send the prompt in file f as it is, no model
#   QUERY=f           the query for the prompt-writing model (default: chain_query.md)
#   RUN=name          put the output under grok/chain/name/ (default: grok/chain/)
#   RENDER=0          skip the fframes build and render at the end
#
# Keys: engine: $NTGT_ENGINE_API_KEY, else ENGINE_API_KEY from ntgt-platform/.env;
#       xAI: $XAI_API_KEY.
# Output: grok/chain/<variant>/{prompt_request.json,prompt_response.json,
#         image_prompt.txt,image_request.json,image_response.json,
#         revised_prompt.txt,image.png,window.png}
set -u
V=${1:-mode_s}
HERE=${0:A:h}
BASE=$HERE/..
OUT=$HERE/chain/${RUN:+$RUN/}$V
PROJECT=$HERE/chain/${RUN:+$RUN/}project-$V
IMAGE_MODEL=${IMAGE_MODEL:-grok-imagine-image-quality}
XAI=${XAI_API_KEY:-}
[[ -n $XAI ]] || { echo "no xAI key"; exit 1; }
mkdir -p $OUT

# 1. the image prompt
if [[ -n ${PROMPT_FILE:-} ]]; then
  cp $PROMPT_FILE $OUT/image_prompt.txt
else
cp ${QUERY:-$HERE/chain_query.md} $OUT/query.md
case $V in
  mode_s)
    KEY=${NTGT_ENGINE_API_KEY:-$(grep -E '^ENGINE_API_KEY=' $HOME/Desktop/Coding_Projects/ntgt-platform/.env | head -1 | cut -d= -f2- | tr -d "\"'\r")}
    python3 -I -c 'import json,sys; print(json.dumps({"query": open(sys.argv[1]).read(), "mode": "mode_s", "provider": "xai", "tier": "top@low"}))' \
      $OUT/query.md > $OUT/prompt_request.json
    curl -s -m 7200 -X POST ${NTGT_ENGINE_URL:-http://127.0.0.1:8081}/evaluate -H 'Content-Type: application/json' -H "X-Api-Key: $KEY" \
      --data-binary @$OUT/prompt_request.json -o $OUT/prompt_response.json -w "$V prompt: http %{http_code} · %{time_total} s\n"
    ;;
  direct)
    python3 -I -c 'import json,sys; print(json.dumps({"model": "grok-4.7", "temperature": 0.7, "messages": [{"role": "user", "content": open(sys.argv[1]).read()}]}))' \
      $OUT/query.md > $OUT/prompt_request.json
    curl -s -m 7200 -X POST https://api.x.ai/v1/chat/completions -H 'Content-Type: application/json' -H "Authorization: Bearer $XAI" \
      --data-binary @$OUT/prompt_request.json -o $OUT/prompt_response.json -w "$V prompt: http %{http_code} · %{time_total} s\n"
    ;;
  *) echo "variant: mode_s or direct"; exit 1 ;;
esac

# The prompt as the model wrote it: the ```text block, else the whole reply.
python3 -I - $OUT/prompt_response.json $OUT/image_prompt.txt <<'PY' || { echo "$V: no prompt in the reply"; exit 1; }
import json, re, sys
data = json.load(open(sys.argv[1]))
if "choices" in data:
    text = data["choices"][0]["message"]["content"] or ""
else:
    text = data.get("response", "")
    try:
        text = json.loads(text).get("final_text", text)
    except (ValueError, AttributeError):
        pass
blocks = re.findall(r"```(?:text)?[^\n]*\n(.*?)```", text, re.S)
prompt = (max(blocks, key=len) if blocks else text).strip()
if not prompt:
    sys.exit(1)
open(sys.argv[2], "w").write(prompt)
PY
fi

# 2. the image, from that prompt unchanged
img() { # $1 = extra JSON fields
  python3 -I -c 'import json,sys; d={"model": sys.argv[2], "prompt": open(sys.argv[1]).read(), "n": 1, "response_format": "b64_json"}; d.update(json.loads(sys.argv[3])); print(json.dumps(d))' \
    $OUT/image_prompt.txt $IMAGE_MODEL "$1" > $OUT/image_request.json
  curl -s -m 900 -X POST https://api.x.ai/v1/images/generations -H 'Content-Type: application/json' -H "Authorization: Bearer $XAI" \
    --data-binary @$OUT/image_request.json -o $OUT/image_response.json -w "%{http_code}"
}
for IMAGE_MODEL in $IMAGE_MODEL grok-imagine-image; do
  code=$(img '{"aspect_ratio": "2:3", "resolution": "2k"}')
  [[ $code == 200 ]] || code=$(img '{"aspect_ratio": "2:3"}')
  [[ $code == 200 ]] || code=$(img '{}')
  echo "$V image ($IMAGE_MODEL): http $code"
  [[ $code == 200 ]] && break
done
python3 -I - $OUT <<'PY' || { echo "$V: no image in the reply"; exit 1; }
import base64, json, sys
out = sys.argv[1]
item = json.load(open(f"{out}/image_response.json"))["data"][0]
open(f"{out}/image.png", "wb").write(base64.b64decode(item["b64_json"]))
open(f"{out}/revised_prompt.txt", "w").write(item.get("revised_prompt") or "")
PY

# 3. fframes: the picture, frame 0
[[ ${RENDER:-1} == 0 ]] && { echo "$V image: $OUT/image.png"; exit 0; }
mkdir -p $PROJECT/src $PROJECT/.cargo
sed "s/^name = \"golden-spurs\"/name = \"golden-spurs-chain-${V//_/-}\"/" $BASE/Cargo.toml > $PROJECT/Cargo.toml
cp $BASE/.cargo/config.toml $PROJECT/.cargo/
cp $BASE/src/main.rs $PROJECT/src/
base64 -i $OUT/image.png | tr -d '\n' > $PROJECT/image.b64
cat > $PROJECT/src/lib.rs <<'RS'
//! The generated window image as frame 0, fitted into a 1200 x 1800 picture.

use fframes::{AudioMap, Color, Duration, FFramesContext, Frame, Svgr, Video};
use std::sync::OnceLock;

pub const WIDTH: usize = 1200;
pub const HEIGHT: usize = 1800;

static SVG: OnceLock<String> = OnceLock::new();

#[derive(Debug)]
pub struct Window;

impl Video for Window {
    const FPS: usize = 30;
    const WIDTH: usize = WIDTH;
    const HEIGHT: usize = HEIGHT;
    const BACKGROUND_COLOR: Color = Color::BLACK;

    fn duration(&self) -> Duration<'_> {
        Duration::Frames(1)
    }
    fn audio(&self) -> AudioMap<'_> {
        AudioMap::none()
    }
    fn render_frame<'a>(&'a self, _frame: Frame, _ctx: &FFramesContext<'a, '_>) -> Svgr<'a> {
        let svg = SVG.get_or_init(|| {
            format!(
                r#"<svg xmlns="http://www.w3.org/2000/svg" width="{WIDTH}" height="{HEIGHT}"><rect width="{WIDTH}" height="{HEIGHT}" fill="black"/><image href="data:image/png;base64,{}" x="0" y="0" width="{WIDTH}" height="{HEIGHT}" preserveAspectRatio="xMidYMid meet"/></svg>"#,
                include_str!("../image.b64")
            )
        });
        Svgr::from_svg_string(svg.clone()).unwrap_or_else(|_| Svgr::empty())
    }
}
RS
cd $PROJECT
if cargo build --release 2> $OUT/build.log && cargo run --release -q --bin golden-spurs-chain-${V//_/-} -- frame 0 -o $OUT/frames > $OUT/render.log 2>&1; then
  cp $OUT/frames/0.png $OUT/window.png && echo "$V picture: $OUT/window.png"
else
  echo "$V: build or render failed: $OUT/build.log"
fi
