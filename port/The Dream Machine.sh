#!/bin/bash

XDG_DATA_HOME=${XDG_DATA_HOME:-$HOME/.local/share}

if [ -d "/opt/system/Tools/PortMaster/" ]; then
  controlfolder="/opt/system/Tools/PortMaster"
elif [ -d "/opt/tools/PortMaster/" ]; then
  controlfolder="/opt/tools/PortMaster"
elif [ -d "$XDG_DATA_HOME/PortMaster/" ]; then
  controlfolder="$XDG_DATA_HOME/PortMaster"
else
  controlfolder="/roms/ports/PortMaster"
fi

source $controlfolder/control.txt
get_controls

[ -f "${controlfolder}/mod_${CFW_NAME}.txt" ] && source "${controlfolder}/mod_${CFW_NAME}.txt"
GAMEDIR="/$directory/ports/dreammachine"
BINARY=ruffle-native-multifile.aarch64.lazy
mkdir -p "$GAMEDIR/storage" "$GAMEDIR/logs"
cd "$GAMEDIR" || exit 1
exec > >(tee "$GAMEDIR/log.txt") 2>&1
printf 'The Dream Machine Ruffle test: %s\n' "$(date)"
[ -f "$GAMEDIR/gamedata/movie.swf" ] || { echo 'Missing gamedata/movie.swf'; exit 1; }
# Upstream's multi-file frontend has a fixed content root. Expose SD data
# using a symlink, not a 300 MB copy into the handheld's RAM-backed /tmp.
# A run that dies mid-shutdown can leave /tmp/gdata_ behind: either our own
# symlink (reuse it), or a real directory holding that run's final
# SharedObject flush (save data). Never refuse to start over it: migrate any
# saves from a stale real directory into the persistent SD tree first.
if [ -L /tmp/gdata_ ]; then
  if [ "$(readlink /tmp/gdata_)" != "$GAMEDIR/gamedata" ]; then
    echo "Replacing foreign symlink /tmp/gdata_ -> $(readlink /tmp/gdata_)"
    rm /tmp/gdata_
  fi
elif [ -d /tmp/gdata_ ]; then
  echo 'Stale content root /tmp/gdata_ (directory); migrating saves to SD.'
  if [ -d /tmp/gdata_/storage ]; then
    mkdir -p "$GAMEDIR/gamedata/storage"
    ( cd /tmp/gdata_/storage && find . -type f ) | while IFS= read -r rel; do
      rel="${rel#./}"
      src="/tmp/gdata_/storage/$rel"
      dst="$GAMEDIR/gamedata/storage/$rel"
      if [ -e "$dst" ] && cmp -s "$src" "$dst"; then continue; fi
      mkdir -p "$(dirname "$dst")"
      [ -e "$dst" ] && cp -p "$dst" "$dst.pre-rescue"
      cp -p "$src" "$dst"
      echo "  rescued $rel"
    done
    sync
  fi
  rm -rf /tmp/gdata_
elif [ -e /tmp/gdata_ ]; then
  echo 'Removing unexpected file at /tmp/gdata_; recreating content root.'
  rm -f /tmp/gdata_
fi
[ -L /tmp/gdata_ ] || ln -s "$GAMEDIR/gamedata" /tmp/gdata_ || exit 1
cleanup() {
  [ "$(readlink /tmp/gdata_ 2>/dev/null)" = "$GAMEDIR/gamedata" ] && rm /tmp/gdata_
  [ -n "${helper_pid:-}" ] && kill "$helper_pid" 2>/dev/null || true
  declare -F dream_gpu_restore >/dev/null && dream_gpu_restore
}
trap cleanup EXIT
source "$GAMEDIR/gpu-governor.sh"
# GPU pin improves redraw cadence, but the first teardown hung the device.
# Keep this candidate opt-in until clean exit is proven on the real handheld.
export DREAM_GPU_PERFORMANCE="${DREAM_GPU_PERFORMANCE:-0}"
dream_gpu_enable
# The native frontend supplies its own controller-driven cursor and A click.
# gptokeyb2 only supplies the exit helper, avoiding duplicate synthetic clicks.
if [ -z "${sdl_controllerconfig:-}" ]; then
  sdl_controllerconfig="$(grep '^0300a3845e0400008e02000014010000,' "$controlfolder/gamecontrollerdb.txt")"
fi
export SDL_GAMECONTROLLERCONFIG="$sdl_controllerconfig"
export SDL_JOYSTICK_ALLOW_BACKGROUND_EVENTS=1
# Diagnostic traces are opt-in (defaults off): export DREAM_INPUT_TRACE=1,
# DREAM_LOAD_TRACE=1, DREAM_TEXTURE_TRACE=1, DREAM_LOOP_TRACE=1 to investigate
# input, imported-asset loading, texture decode or main-loop pacing.
# DREAM_INPUT_TRACE_AXIS additionally logs every axis motion (chatty).
export DREAM_INPUT_TRACE="${DREAM_INPUT_TRACE:-0}"
export DREAM_INPUT_TRACE_AXIS="${DREAM_INPUT_TRACE_AXIS:-0}"
export DREAM_LOAD_TRACE="${DREAM_LOAD_TRACE:-0}"
export DREAM_TEXTURE_TRACE="${DREAM_TEXTURE_TRACE:-0}"
export DREAM_LOOP_TRACE="${DREAM_LOOP_TRACE:-0}"
echo "traces: input=$DREAM_INPUT_TRACE axis=$DREAM_INPUT_TRACE_AXIS load=$DREAM_LOAD_TRACE texture=$DREAM_TEXTURE_TRACE loop=$DREAM_LOOP_TRACE"
# Filter-cache diagnostics (opt-in): DREAM_FILTER_TRACE=1 dumps each bitmap
# cache texture (object / scratch copy / composited result) to
# DREAM_FILTER_DUMP_DIR as PPM + pixel stats; DREAM_FILTER_MODE=object skips
# the filter composite to isolate what the cache render alone produces.
export DREAM_FILTER_TRACE="${DREAM_FILTER_TRACE:-0}"
export DREAM_FILTER_DUMP_DIR="${DREAM_FILTER_DUMP_DIR:-$GAMEDIR/logs}"
export DREAM_FILTER_MODE="${DREAM_FILTER_MODE:-full}"
echo "filter_diag: trace=$DREAM_FILTER_TRACE mode=$DREAM_FILTER_MODE dir=$DREAM_FILTER_DUMP_DIR"
export DREAM_LOG_LEVEL="${DREAM_LOG_LEVEL:-warn}"
export RUST_BACKTRACE=1
export TOR_AUDIO_SAMPLES=1024
# Reversible lossless bitmap backing; set DREAM_BITMAP_CACHE=0 to A/B RAM.
if [ "${DREAM_BITMAP_CACHE:-1}" = 1 ]; then
  mkdir -p "$GAMEDIR/bitmap-cache"
  export TOR_BITMAP_CACHE_DIR="$GAMEDIR/bitmap-cache"
  echo "lossless_bitmap_cache=$TOR_BITMAP_CACHE_DIR"
else
  unset TOR_BITMAP_CACHE_DIR
fi
# MSAA sample count for the glow renderer (DREAM_MSAA_SAMPLES=1|2|4).
# 4 = baseline (stage quality HIGH); 1 = single-sample FBO: no multisampling,
# but the FBO + resolve pipeline is kept because stencil-based Flash masks
# need it (the KMSDRM default framebuffer has no stencil buffer).
# NOTE: Mali-G57 silently rounds 2x requests up to 4x, so only 1/4 matter here.
# Reversible A/B knob.
export DREAM_MSAA_SAMPLES="${DREAM_MSAA_SAMPLES:-1}"
echo "msaa_samples=$DREAM_MSAA_SAMPLES"
if [ -r /proc/asound/cards ] && grep -q '\[audiocodec' /proc/asound/cards; then
  export ALSA_CONFIG_PATH="$GAMEDIR/asound.conf"
fi
# The native frontend handles Start+Select exit; no helper is needed.
pm_platform_helper "$GAMEDIR/$BINARY"
"$GAMEDIR/$BINARY"
rc=$?
echo "game_exit_code=$rc"
pm_finish
exit "$rc"
