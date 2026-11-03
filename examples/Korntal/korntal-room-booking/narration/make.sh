#!/bin/zsh
# Narration clips from lines.tsv: one 48 kHz mono WAV per sentence in ../media,
# their durations in durations.tsv (id, seconds). Voice: macOS `say`.
set -e
here=${0:A:h}
media=$here/../media
tmp=$(mktemp -d)
: > $here/durations.tsv
while IFS=$'\t' read -r id voice shown spoken; do
  [[ -z $id ]] && continue
  say -v "$voice" -r 168 -o $tmp/$id.aiff -- "$spoken"
  ffmpeg -v error -y -i $tmp/$id.aiff -ac 1 -ar 48000 -af "adelay=0,apad=pad_dur=0.05" $media/$id.wav
  d=$(ffprobe -v error -show_entries format=duration -of csv=p=0 $media/$id.wav)
  print -r -- "$id	$d" >> $here/durations.tsv
done < $here/lines.tsv
rm -rf $tmp
