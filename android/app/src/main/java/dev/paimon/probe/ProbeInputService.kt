package dev.paimon.probe

import android.os.Process
import java.util.concurrent.TimeUnit

/** Runs in the Shizuku shell process, never in the application UID. */
class ProbeInputService : IProbeInput.Stub() {
    @Volatile private var process: java.lang.Process? = null
    private val gate = Any()
    private var cancelled = true

    private fun requireShellIdentity() {
        check(Process.myUid() == 2000) { "This experiment requires Shizuku shell UID 2000" }
    }

    override fun uid(): Int = Process.myUid()

    override fun begin() { requireShellIdentity(); synchronized(gate) { cancelled = false } }

    override fun foregroundPackage(): String {
        requireShellIdentity()
        // Query the resumed Activity, not the last accessibility window (which may be an overlay).
        // This fixed command contains no user-provided shell text. grep bounds the output so
        // waiting before reading cannot fill the process pipe with the full task dump.
        val child = ProcessBuilder("/system/bin/sh", "-c",
            "/system/bin/dumpsys activity activities | /system/bin/grep -m 1 topResumedActivity")
            .redirectErrorStream(true).start()
        return try {
            if (!child.waitFor(1, TimeUnit.SECONDS) || child.exitValue() != 0) ""
            else Regex("\\bu\\d+\\s+([A-Za-z0-9_.]+)/").find(child.inputStream.bufferedReader().use { it.readText() })
                ?.groupValues?.get(1).orEmpty()
        } finally {
            child.destroyForcibly()
        }
    }

    override fun tap(x: Int, y: Int, expectedPackage: String): Int {
        requireShellIdentity()
        require(x in 0..16383 && y in 0..16383)
        require(expectedPackage.isNotBlank())
        if (foregroundPackage() != expectedPackage) return -6
        val child = synchronized(gate) {
            if (cancelled) return -5
            check(process == null) { "Input command already in flight" }
            ProcessBuilder("/system/bin/input", "touchscreen", "tap", x.toString(), y.toString())
                .redirectErrorStream(true).start().also { process = it }
        }
        return try {
            if (!child.waitFor(2, TimeUnit.SECONDS)) {
                child.destroyForcibly()
                -2
            } else child.exitValue()
        } finally {
            child.destroy()
            synchronized(gate) { if (process === child) process = null }
        }
    }

    override fun cancel() {
        requireShellIdentity()
        synchronized(gate) { cancelled = true; process?.destroyForcibly() }
    }

    override fun destroy() { process?.destroyForcibly(); kotlin.system.exitProcess(0) }
}
