#!/system/bin/sh
# Phone-local, create-only fault injection. Never grabs or writes touch events.
set -u
helper=/data/local/tmp/paimon-touch-helper
device=${1:?pass the inspected physical evdev path}
log=/data/local/tmp/paimon-watchdog-check-$$.jsonl
worker=''
supervisor=''
completed=0
child_of() {
    ps -p "$1" -o PPID | awk -v expected="$2" 'NR > 1 && $1 == expected {found=1} END {exit !found}'
}
cleanup() {
    status=$?
    trap - EXIT HUP INT TERM
    if [ "$completed" -ne 1 ] && [ -n "$supervisor" ] && child_of "$supervisor" "$$"; then
        if [ -n "$worker" ] && child_of "$worker" "$supervisor"; then
            kill -KILL "$worker" 2>/dev/null || true
        fi
        kill -KILL "$supervisor" 2>/dev/null || true
    fi
    if [ -n "$supervisor" ]; then wait "$supervisor" 2>/dev/null || true; fi
    exit "$status"
}
trap cleanup EXIT
trap 'exit 129' HUP
trap 'exit 130' INT
trap 'exit 143' TERM
sleep 15 | "$helper" create --device "$device" --seconds 15 >"$log" 2>&1 &
supervisor=$!
attempt=0
while ! grep -q '"event":"created"' "$log"; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 50 ]; then cat "$log"; echo 'create did not become ready' >&2; exit 1; fi
    sleep 0.1
done
worker=$(ps -A -o PID,PPID | awk -v parent="$supervisor" '$2 == parent {print $1}')
case "$worker" in ''|*[!0-9]*) cat "$log"; echo 'expected exactly one worker child' >&2; exit 1 ;; esac
if ! ps -p "$worker" -o ARGS | grep -Fq -- 'paimon-touch-helper --worker create'; then
    ps -p "$worker" -o PID,PPID,ARGS
    echo 'worker command did not match create-only experiment' >&2; exit 1
fi
printf '{"event":"fault_injected","supervisor":%s,"worker":%s,"signal":"STOP","touchGrabbed":false}\n' "$supervisor" "$worker"
started=$(date +%s)
kill -STOP "$worker" || exit 1
attempt=0
while ! grep -q '"reason":"HeartbeatExpired"' "$log"; do
    attempt=$((attempt + 1))
    if [ "$attempt" -ge 60 ]; then cat "$log"; echo 'watchdog recovery not observed within 6 seconds' >&2; exit 1; fi
    sleep 0.1
done
elapsed=$(( $(date +%s) - started ))
wait "$supervisor"
result=$?
cat "$log"
printf '{"event":"watchdog_check","exitCode":%s,"observedSeconds":%s}\n' "$result" "$elapsed"
if [ "$result" -ne 2 ] || [ "$elapsed" -gt 6 ]; then exit 1; fi
completed=1
