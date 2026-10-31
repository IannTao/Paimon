package dev.paimon.probe

import android.content.Context
import android.util.AtomicFile
import java.io.File
import org.json.JSONObject

object NavigationReports {
    @Synchronized fun save(context: Context, report: JSONObject) {
        val file = AtomicFile(File(context.filesDir, "navigation-latest.json"))
        val output = file.startWrite()
        try { output.write(report.toString().toByteArray()); file.finishWrite(output) }
        catch (error: Throwable) { file.failWrite(output); throw error }
    }
    @Synchronized fun latest(context: Context): JSONObject =
        AtomicFile(File(context.filesDir, "navigation-latest.json")).openRead().use {
            val bytes = it.readNBytes(2 * 1024 * 1024 + 1)
            check(bytes.size <= 2 * 1024 * 1024)
            JSONObject(String(bytes, Charsets.UTF_8))
        }
}
