package dev.paimon.probe

import android.os.Process
import android.os.SystemClock
import android.view.InputDevice
import android.view.InputEvent
import android.view.MotionEvent
import java.nio.ByteBuffer
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** One short gesture per fresh screen lease; all releases are owned in this shell process. */
class NavigationInputService : INavigationInput.Stub() {
    private val gate = Any()
    private val screen = ShellMiniMapCapture()
    private val watchdog = Executors.newScheduledThreadPool(2)
    private var runId = 0L
    private var lastSerial = 0L
    private var capturedAt = 0L
    private var consumedFrame = 0L
    private var held = false
    private var downAt = 0L
    private var x = 0f
    private var y = 0f
    private val manager by lazy { Class.forName("android.hardware.input.InputManager").getMethod("getInstance").invoke(null) }
    private val injector by lazy { manager.javaClass.getMethod("injectInputEvent", InputEvent::class.java, Int::class.javaPrimitiveType) }
    private fun requireShell() { check(Process.myUid() == 2000) { "只接受 shell Shizuku" } }
    override fun foregroundPackage(): String { requireShell(); return ShellForeground.resumedPackage() }
    override fun begin(runId: Long) {
        requireShell()
        synchronized(gate) { check(runId > 0 && this.runId == 0L && !held); this.runId = runId }
    }
    override fun capture(runId: Long): ByteArray {
        requireShell()
        synchronized(gate) { check(this.runId == runId && runId > 0 && !held) }
        val packet = screen.capture()
        synchronized(gate) { check(this.runId == runId && !held); capturedAt = ByteBuffer.wrap(packet, 0, 8).long }
        return packet
    }
    private fun inject(action: Int, px: Float, py: Float): Boolean {
        val event = MotionEvent.obtain(downAt, SystemClock.uptimeMillis(), action, px, py, 0)
        event.source = InputDevice.SOURCE_TOUCHSCREEN
        return try { injector.invoke(manager, event, 0) == true } finally { event.recycle() }
    }
    private fun release(action: Int = MotionEvent.ACTION_CANCEL): Boolean {
        if (!held) return true
        val accepted = inject(action, x, y)
        if (accepted) held = false
        return accepted
    }
    override fun execute(runId: Long, serial: Long, kind: Int, dx: Int, durationMs: Int, capturedAt: Long): LongArray {
        requireShell()
        require((kind == 1 && dx != 0 && dx in -240..240 && durationMs == 180) ||
            (kind == 2 && dx == 0 && durationMs in 120..400)) { "输入命令超出已验证范围" }
        check(foregroundPackage() == MovementService.GAME) { "原神不在前台" }
        val now = SystemClock.elapsedRealtime()
        synchronized(gate) {
            check(this.runId == runId && runId > 0 && !held && serial > lastSerial)
            check(this.capturedAt == capturedAt && capturedAt > consumedFrame && now >= capturedAt && now - capturedAt <= 1500) { "输入画面许可已过期或已消费" }
            lastSerial = serial; consumedFrame = capturedAt
        }
        val hardStop = watchdog.schedule({
            synchronized(gate) {
                if (this.runId == runId && lastSerial == serial && held) { this.runId = 0; release() }
            }
        }, 650, TimeUnit.MILLISECONDS)
        val foregroundWatch = watchdog.scheduleWithFixedDelay({
            if (runCatching { foregroundPackage() != MovementService.GAME }.getOrDefault(true)) {
                synchronized(gate) { if (this.runId == runId && lastSerial == serial && held) { this.runId = 0; release() } }
            }
        }, 80, 100, TimeUnit.MILLISECONDS)
        var success = false
        var released: Boolean
        var started = 0L
        try {
            synchronized(gate) {
                check(this.runId == runId)
                downAt = SystemClock.uptimeMillis(); started = downAt
                x = if (kind == 1) 1450f else 455f
                y = if (kind == 1) 500f else 850f
                held = true
                check(inject(MotionEvent.ACTION_DOWN, x, y))
            }
            val start = SystemClock.elapsedRealtime()
            while (true) {
                val elapsed = SystemClock.elapsedRealtime() - start
                if (elapsed >= durationMs) break
                synchronized(gate) {
                    check(this.runId == runId && held && SystemClock.elapsedRealtime() - capturedAt <= 1500)
                    x = if (kind == 1) 1450f + dx * (elapsed.toFloat() / durationMs).coerceIn(0f, 1f) else 455f
                    y = if (kind == 1) 500f else 730f
                    check(inject(MotionEvent.ACTION_MOVE, x, y))
                }
                Thread.sleep(12)
            }
            synchronized(gate) {
                check(this.runId == runId && held)
                if (kind == 1) { x = 1450f + dx; check(inject(MotionEvent.ACTION_MOVE, x, y)) }
            }
            success = true
        } finally {
            synchronized(gate) { released = release(if (success) MotionEvent.ACTION_UP else MotionEvent.ACTION_CANCEL) }
            hardStop.cancel(false)
            foregroundWatch.cancel(true)
        }
        return longArrayOf(if (success) 1 else 0, if (released) 1 else 0, started, SystemClock.uptimeMillis())
    }
    override fun cancel(runId: Long): Boolean {
        requireShell()
        synchronized(gate) {
            if (this.runId != 0L && this.runId != runId) return !held
            this.runId = 0; val released = release(); screen.cancel(); return released
        }
    }
    override fun holdingSince(runId: Long): Long { requireShell(); return synchronized(gate) { if (this.runId == runId && held) downAt else 0L } }
    override fun destroy() { synchronized(gate) { runId = 0; runCatching { release() } }; screen.close(); watchdog.shutdownNow(); kotlin.system.exitProcess(0) }
}
