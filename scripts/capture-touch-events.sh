#!/system/bin/sh
# Read-only diagnostic capture, scoped to the receiver window. No EVIOCGRAB.
set -u
device=${1:?pass the inspected physical evdev path}
capture=''
cleanup() {
    status=$?
    trap - EXIT HUP INT TERM
    if [ -n "$capture" ]; then
        if ps -p "$capture" -o PPID | awk -v owner="$$" 'NR > 1 && $1 == owner {found=1} END {exit !found}'; then
            kill -TERM "$capture" 2>/dev/null || true
        fi
        wait "$capture" 2>/dev/null || true
    fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
focused_receiver() {
    dumpsys window displays | grep 'mCurrentFocus=' | head -n 1 | grep -Fq 'dev.paimon.touchreceiver/'
}
if ! focused_receiver; then echo '{"event":"capture_refused","reason":"receiver_not_focused"}'; exit 1; fi
printf '{"event":"physical_capture_started","device":"%s","grabbed":false}\n' "$device"
getevent -t "$device" &
capture=$!
started=$(date +%s)
while kill -0 "$capture" 2>/dev/null; do
    sleep 0.5
    if ! focused_receiver; then echo '{"event":"capture_ended","reason":"focus_changed"}'; exit 0; fi
    if [ "$(( $(date +%s) - started ))" -ge 600 ]; then echo '{"event":"capture_ended","reason":"ten_minute_limit"}'; exit 0; fi
done
wait "$capture"
