package dev.paimon.probe

import android.content.Context
import android.util.AtomicFile
import org.json.JSONObject
import java.io.File

/** User-visible task reports; atomic publication keeps running status readable. */
object MovementReports {
    @Synchronized
    fun latest(context: Context): JSONObject = AtomicFile(File(context.filesDir, "movement-latest.json"))
        .openRead().bufferedReader().use { JSONObject(it.readText()) }

    @Synchronized
    fun save(context: Context, report: JSONObject, publishLatest: Boolean) {
        val reports = File(context.filesDir, "movement-reports").apply { mkdirs() }
        val body = report.toString(2).toByteArray(Charsets.UTF_8)
        write(File(reports, "${report.getLong("runId")}.json"), body)
        if (publishLatest) write(File(context.filesDir, "movement-latest.json"), body)
    }

    private fun write(destination: File, body: ByteArray) {
        val file = AtomicFile(destination)
        val stream = file.startWrite()
        try {
            stream.write(body)
            file.finishWrite(stream)
        } catch (error: Exception) {
            file.failWrite(stream)
            throw error
        }
    }
}
