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
if [ -e /tmp/gdata_ ] || [ -L /tmp/gdata_ ]; then
  echo 'Content root /tmp/gdata_ already exists, refusing to replace it.'
  exit 1
fi
ln -s "$GAMEDIR/gamedata" /tmp/gdata_ || exit 1
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
