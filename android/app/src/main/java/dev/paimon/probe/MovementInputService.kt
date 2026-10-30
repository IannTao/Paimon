package dev.paimon.probe

import android.graphics.Bitmap
import android.os.Process
import android.os.SystemClock
import android.view.InputDevice
import android.view.InputEvent
import android.view.MotionEvent
import java.io.DataInputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** Shell-only input owner. Every hold ends locally even if the app stops responding. */
class MovementInputService : IMovementInput.Stub() {
    private val gate = Any()
    private val watchdog = Executors.newSingleThreadScheduledExecutor()
    private var runId = 0L
    private var expectedPackage = ""
    private var held = false
    private var downTime = 0L
    private var used = false
    private var permitUntil = 0L
    @Volatile private var captureProcess: java.lang.Process? = null
    private val manager by lazy {
        Class.forName("android.hardware.input.InputManager").getMethod("getInstance").invoke(null)
    }
    private val injector by lazy {
        manager.javaClass.getMethod("injectInputEvent", InputEvent::class.java, Int::class.javaPrimitiveType)
    }

    private fun requireShell() { check(Process.myUid() == 2000) { "Requires shell Shizuku UID 2000" } }

    override fun foregroundPackage(): String {
        requireShell()
        return ShellForeground.resumedPackage()
    }

    override fun begin(id: Long, game: String) {
        requireShell()
        require(id > 0 && game == "com.miHoYo.Yuanshen") { "Only the baseline Chinese game package is supported" }
        synchronized(gate) {
            check(runId == 0L && !held) { "Previous run must be released first" }
            runId = id; expectedPackage = game; used = false; permitUntil = 0
        }
    }

    private fun current(id: Long): Boolean = synchronized(gate) { runId == id && id != 0L }

    override fun holdingSince(id: Long): Long {
        requireShell()
        return synchronized(gate) { if (runId == id && held) downTime else 0L }
    }

    override fun capture(id: Long): ByteArray {
        requireShell()
        check(current(id) && foregroundPackage() == expectedPackage) { "Game is not foreground" }
        val capturedAt = SystemClock.elapsedRealtime()
        val child = synchronized(gate) {
            check(runId == id && captureProcess == null) { "Capture cancelled or already in flight" }
            ProcessBuilder("/system/bin/screencap").start().also { captureProcess = it }
        }
        val timeout = watchdog.schedule({ child.destroyForcibly() }, 1200, TimeUnit.MILLISECONDS)
        try {
            // Android 14 screencap emits four native-endian uint32s, then packed
            // rows without stride padding. Reject unknown formats before allocation.
            val raw = DataInputStream(child.inputStream).use { stream ->
                val header = ByteArray(16).also { stream.readFully(it) }
                val layout = ByteBuffer.wrap(header).order(ByteOrder.LITTLE_ENDIAN)
                check(layout.int == 2480 && layout.int == 1116) { "Requires 2480 x 1116 landscape" }
                check(layout.int in 1..2 && layout.int in 1..2) { "Unsupported screen pixel format or colorspace" }
                ByteArray(2480 * 1116 * 4).also {
                    stream.readFully(it)
                    check(stream.read() == -1) { "Unexpected screenshot trailing data" }
                }
            }
            val exited = child.waitFor(300, TimeUnit.MILLISECONDS)
            check(exited && child.exitValue() == 0) {
                "Screenshot failed: exit=${if (exited) child.exitValue() else "pending"}, elapsedMs=${SystemClock.elapsedRealtime() - capturedAt}"
            }
            val full = Bitmap.createBitmap(2480, 1116, Bitmap.Config.ARGB_8888)
            try {
                full.copyPixelsFromBuffer(ByteBuffer.wrap(raw))
                val scaled = Bitmap.createScaledBitmap(full, 496, 224, true)
                try {
                    val pixels = IntArray(496 * 224)
                    scaled.getPixels(pixels, 0, 496, 0, 0, 496, 224)
                    val packet = ByteBuffer.allocate(8 + pixels.size * 3).putLong(capturedAt)
                    for (color in pixels) {
                        packet.put((color shr 16).toByte()).put((color shr 8).toByte()).put(color.toByte())
                    }
                    check(current(id) && foregroundPackage() == expectedPackage) { "Foreground changed during capture" }
                    synchronized(gate) {
                        check(runId == id)
                        permitUntil = capturedAt + 1500
                    }
                    return packet.array()
                } finally { if (scaled !== full) scaled.recycle() }
            } finally { full.recycle() }
        } finally {
            timeout.cancel(false); child.destroyForcibly(); captureProcess = null
        }
    }

    // Caller holds gate. Async injection bounds lock ownership to the Binder call,
    // and uses one downTime and one pointer throughout the joystick gesture.
    private fun inject(action: Int, x: Float, y: Float): Boolean {
        val event = MotionEvent.obtain(downTime, SystemClock.uptimeMillis(), action, x, y, 0)
        event.source = InputDevice.SOURCE_TOUCHSCREEN
        return try { injector.invoke(manager, event, 0) == true } finally { event.recycle() }
    }

    private fun release(): Boolean {
        if (!held) return true
        val released = inject(MotionEvent.ACTION_CANCEL, 455f, 730f)
        if (released) held = false
        return released
    }

    override fun move(id: Long): Boolean {
        requireShell()
        if (!current(id) || foregroundPackage() != expectedPackage) return false
        val hardStop = watchdog.schedule({ cancel(id) }, 1200, TimeUnit.MILLISECONDS)
        var success = false
        try {
            synchronized(gate) {
                check(runId == id && !used && SystemClock.elapsedRealtime() < permitUntil)
                used = true; downTime = SystemClock.uptimeMillis()
                // Record ownership before injection so a partial failure still releases.
                held = true
                check(inject(MotionEvent.ACTION_DOWN, 455f, 850f)) { "Joystick down rejected" }
            }
            val until = SystemClock.elapsedRealtime() + 900
            var nextForegroundCheck = 0L
            while (SystemClock.elapsedRealtime() < until) {
                if (SystemClock.elapsedRealtime() >= nextForegroundCheck) {
                    check(foregroundPackage() == expectedPackage) { "Foreground changed" }
                    nextForegroundCheck = SystemClock.elapsedRealtime() + 100
                }
                synchronized(gate) {
                    check(runId == id && held && SystemClock.elapsedRealtime() < permitUntil)
                    check(inject(MotionEvent.ACTION_MOVE, 455f, 730f)) { "Joystick move rejected" }
                }
                Thread.sleep(16)
            }
            success = true
        } finally {
            synchronized(gate) { success = release() && success }
            hardStop.cancel(false)
        }
        return success
    }

    // -1: release failed, 0: already up, positive: held input was cancelled at this uptime.
    override fun cancel(id: Long): Long {
        requireShell()
        synchronized(gate) {
            if (runId != id) return if (held) -1L else 0L
            val releasedAt = if (held) SystemClock.uptimeMillis() else 0L
            runId = 0; permitUntil = 0
            captureProcess?.destroyForcibly()
            return if (release()) releasedAt else -1L
        }
    }

    override fun destroy() {
        synchronized(gate) { runId = 0; runCatching { release() } }
        captureProcess?.destroyForcibly(); watchdog.shutdownNow()
        kotlin.system.exitProcess(0)
    }
}
