#!/bin/bash
# q6a-monitor.sh — Resource monitoring for Dragon Q6A
#
# Usage: ./scripts/q6a-monitor.sh [interval_seconds]
#
# Monitors: CPU temp, CPU freq, memory, GPU, NPU status

Q6A_HOST="${Q6A_HOST:-radxa@192.168.100.2}"
INTERVAL="${1:-5}"

echo "=== Dragon Q6A Resource Monitor ==="
echo "Host: $Q6A_HOST | Interval: ${INTERVAL}s"
echo ""

while true; do
    ssh "$Q6A_HOST" '
        # CPU temperature (millidegrees → degrees)
        temps=$(cat /sys/class/thermal/thermal_zone*/temp 2>/dev/null | head -5)
        max_temp=0
        for t in $temps; do
            if [ "$t" -gt "$max_temp" ]; then max_temp=$t; fi
        done
        temp_c=$((max_temp / 1000))

        # CPU frequency (kHz → GHz)
        freq_big=$(cat /sys/devices/system/cpu/cpu4/cpufreq/scaling_cur_freq 2>/dev/null)
        freq_little=$(cat /sys/devices/system/cpu/cpu0/cpufreq/scaling_cur_freq 2>/dev/null)
        freq_big_ghz=$(echo "scale=2; ${freq_big:-0} / 1000000" | bc)
        freq_little_ghz=$(echo "scale=2; ${freq_little:-0} / 1000000" | bc)

        # Memory
        mem_total=$(free -m | awk "/Mem:/ {print \$2}")
        mem_used=$(free -m | awk "/Mem:/ {print \$3}")
        mem_pct=$((mem_used * 100 / mem_total))

        # Load
        load=$(cat /proc/loadavg | cut -d" " -f1-3)

        # CDSP (NPU) status
        cdsp=$(cat /sys/bus/platform/drivers/fastrpc/*/subsys_state 2>/dev/null | head -1)

        printf "[%s] CPU: %d°C | A78: %sGHz A55: %sGHz | RAM: %dMB/%dMB (%d%%) | Load: %s | CDSP: %s\n" \
            "$(date +%H:%M:%S)" "$temp_c" "$freq_big_ghz" "$freq_little_ghz" \
            "$mem_used" "$mem_total" "$mem_pct" "$load" "${cdsp:-unknown}"
    '
    sleep "$INTERVAL"
done
