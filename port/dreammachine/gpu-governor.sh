#!/bin/bash
# Port-scoped, reversible native GPU governor lease. No firmware overclock.
dream_gpu_enable() {
  _dream_gpu_changed=0
  _dream_gpu_path="${DREAM_GPU_SYSFS:-/sys/class/devfreq/1800000.gpu}"
  [ "${DREAM_GPU_PERFORMANCE:-1}" = 1 ] || return 0
  [ -r "$_dream_gpu_path/governor" ] && [ -w "$_dream_gpu_path/governor" ] || return 0
  IFS= read -r _dream_gpu_available < "$_dream_gpu_path/available_governors" || return 0
  case " $_dream_gpu_available " in *" performance "*) ;; *) return 0 ;; esac
  IFS= read -r _dream_gpu_old < "$_dream_gpu_path/governor" || return 0
  _dream_gpu_changed=1
  if ! printf '%s\n' performance > "$_dream_gpu_path/governor"; then
    _dream_gpu_changed=0
    echo 'dream_gpu_governor=unchanged (write failed)'
    return 0
  fi
  echo "dream_gpu_governor=performance previous=$_dream_gpu_old"
}
dream_gpu_restore() {
  [ "${_dream_gpu_changed:-0}" = 1 ] || return 0
  local current
  if IFS= read -r current < "$_dream_gpu_path/governor" && [ "$current" = performance ]; then
    if printf '%s\n' "$_dream_gpu_old" > "$_dream_gpu_path/governor"; then
      echo "dream_gpu_governor_restored=$_dream_gpu_old"
    else
      echo 'WARNING: GPU governor restoration failed'
    fi
  else
    echo 'dream_gpu_governor_restore=skipped (setting changed externally)'
  fi
  _dream_gpu_changed=0
}
