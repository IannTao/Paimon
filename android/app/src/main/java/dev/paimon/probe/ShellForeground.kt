package dev.paimon.probe

import android.os.Process
import java.util.concurrent.TimeUnit

/** Only called inside shell UserServices, never from the application UID. */
object ShellForeground {
    fun resumedPackage(): String {
        check(Process.myUid() == 2000) { "Requires shell UID 2000" }
        val child = ProcessBuilder("/system/bin/sh", "-c",
            "/system/bin/dumpsys activity activities | /system/bin/grep -m 1 topResumedActivity")
            .redirectErrorStream(true).start()
        return try {
            if (!child.waitFor(700, TimeUnit.MILLISECONDS) || child.exitValue() != 0) ""
            else Regex("\\bu\\d+\\s+([A-Za-z0-9_.]+)/")
                .find(child.inputStream.bufferedReader().use { it.readText() })?.groupValues?.get(1).orEmpty()
        } finally { child.destroyForcibly() }
    }
}
