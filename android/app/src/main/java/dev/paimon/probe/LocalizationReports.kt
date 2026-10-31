package dev.paimon.probe

import android.content.Context
import android.util.AtomicFile
import org.json.JSONObject
import java.io.File

object LocalizationReports {
    @Synchronized fun save(context: Context, report: JSONObject) {
        val file = AtomicFile(File(context.filesDir, "localization-latest.json"))
        val output = file.startWrite()
        try { output.write(report.toString().toByteArray(Charsets.UTF_8)); file.finishWrite(output) }
        catch (error: Throwable) { file.failWrite(output); throw error }
    }
    @Synchronized fun latest(context: Context): JSONObject =
        AtomicFile(File(context.filesDir, "localization-latest.json")).openRead().use { input ->
            val bytes = input.readNBytes(64 * 1024 + 1)
            check(bytes.size <= 64 * 1024) { "定位记录过大" }
            JSONObject(String(bytes, Charsets.UTF_8))
        }
}
