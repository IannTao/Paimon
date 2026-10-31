package dev.paimon.probe

import android.os.Process
import android.os.SystemClock
import java.io.DataInputStream
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.Executors
import java.util.concurrent.TimeUnit

/** Bounded screen ownership shared by observation and navigation shell adapters. */
class ShellMiniMapCapture {
    private val gate = Any()
    private val watchdog = Executors.newSingleThreadScheduledExecutor()
    @Volatile private var child: java.lang.Process? = null
    @Volatile private var closed = false

    fun capture(): ByteArray {
        check(Process.myUid() == 2000 && !closed) { "需要 shell Shizuku" }
        check(ShellForeground.resumedPackage() == MovementService.GAME) { "原神不在前台" }
        val capturedAt = SystemClock.elapsedRealtime()
        val process = synchronized(gate) {
            check(!closed && child == null) { "截图已取消或仍在处理" }
            ProcessBuilder("/system/bin/screencap").start().also { child = it }
        }
        val timeout = watchdog.schedule({ process.destroyForcibly() }, 1200, TimeUnit.MILLISECONDS)
        try {
            val bytes = DataInputStream(process.inputStream).use { input ->
                val header = ByteArray(16).also { input.readFully(it) }
                val layout = ByteBuffer.wrap(header).order(ByteOrder.LITTLE_ENDIAN)
                check(layout.int == 2480 && layout.int == 1116) { "仅支持 2480×1116 横屏布局" }
                check(layout.int in 1..2 && layout.int in 1..2) { "截图像素格式或色彩空间不支持" }
                ByteArray(2480 * 1116 * 4).also { input.readFully(it); check(input.read() == -1) }
            }
            check(process.waitFor(300, TimeUnit.MILLISECONDS) && process.exitValue() == 0) { "截图失败" }
            check(!closed && ShellForeground.resumedPackage() == MovementService.GAME) { "截图时前台改变" }
            val packet = ByteBuffer.allocate(8 + 264 * 264 * 3).putLong(capturedAt)
            for (y in 28 until 292) for (x in 180 until 444) {
                val at = (y * 2480 + x) * 4
                packet.put(bytes[at]).put(bytes[at + 1]).put(bytes[at + 2])
            }
            return packet.array()
        } finally {
            timeout.cancel(false); process.destroyForcibly()
            synchronized(gate) { if (child === process) child = null }
        }
    }
    fun cancel() { closed = true; synchronized(gate) { child?.destroyForcibly() } }
    fun close() { cancel(); watchdog.shutdownNow() }
}
