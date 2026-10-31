package dev.paimon.probe

import android.content.Context
import android.util.AtomicFile
import java.io.File
import org.json.JSONObject

object NavigationReports {
    private var owner = 0L
    @Synchronized fun claim(runId: Long) { owner = runId }
    @Synchronized fun save(context: Context, report: JSONObject) {
        if (report.getLong("runId") != owner) return
        val bytes = report.toString().toByteArray(Charsets.UTF_8)
        check(bytes.size <= 2 * 1024 * 1024) { "导航报告超过2MiB" }
        val file = AtomicFile(File(context.filesDir, "navigation-latest.json"))
        val output = file.startWrite()
        try { output.write(bytes); file.finishWrite(output) }
        catch (error: Throwable) { file.failWrite(output); throw error }
    }
    @Synchronized fun latest(context: Context): JSONObject =
        AtomicFile(File(context.filesDir, "navigation-latest.json")).openRead().use {
            val bytes = it.readNBytes(2 * 1024 * 1024 + 1)
            check(bytes.size <= 2 * 1024 * 1024)
            JSONObject(String(bytes, Charsets.UTF_8))
        }
}
