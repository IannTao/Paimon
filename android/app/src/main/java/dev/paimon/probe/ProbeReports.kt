package dev.paimon.probe

import android.content.Context
import org.json.JSONObject
import java.io.File
import java.nio.file.Files
import java.nio.file.StandardCopyOption

/** Serializes report updates across old/new service instances and the observer UI. */
object ProbeReports {
    private val observationFields = listOf(
        "gameAccepted", "itemObtained", "touchContinuity", "observerNotes", "observedAt", "overallGate"
    )

    @Synchronized fun saveRun(context: Context, report: JSONObject, makeLatest: Boolean = false) {
        val archive = archive(context, report)
        if (archive.isFile) {
            val previous = JSONObject(archive.readText())
            // A late input result may update command evidence, never erase human observations.
            if (previous.has("observedAt")) {
                observationFields.forEach { field ->
                    if (previous.has(field)) report.put(field, previous.get(field))
                }
            }
        }
        write(archive, report.toString(2))
        val latest = File(context.filesDir, "latest-run-id")
        // Only starting a run changes the pointer. Old callbacks cannot reclaim "latest".
        if (makeLatest) write(latest, report.getString("runId"))
    }

    @Synchronized fun latest(context: Context): JSONObject {
        val pointer = File(context.filesDir, "latest-run-id")
        check(pointer.isFile) { "尚无运行报告" }
        val id = pointer.readText()
        requireValidId(id)
        return JSONObject(File(File(context.filesDir, "probe-reports"), "$id.json").readText())
    }

    @Synchronized fun recordObservation(context: Context, observed: JSONObject): JSONObject {
        val target = archive(context, observed)
        // Keep any newer command completion evidence instead of replacing the whole snapshot.
        val current = JSONObject(target.readText())
        observationFields.forEach { field ->
            if (observed.has(field)) current.put(field, observed.get(field))
        }
        write(target, current.toString(2))
        return current
    }

    private fun archive(context: Context, report: JSONObject): File {
        val id = report.getString("runId")
        requireValidId(id)
        return File(File(context.filesDir, "probe-reports").apply { mkdirs() }, "$id.json")
    }

    private fun requireValidId(id: String) {
        require(id.matches(Regex("[0-9a-f-]{36}"))) { "Invalid report ID" }
    }

    private fun write(destination: File, content: String) {
        val temporary = File(destination.parentFile, destination.name + ".tmp")
        temporary.writeText(content)
        Files.move(temporary.toPath(), destination.toPath(), StandardCopyOption.REPLACE_EXISTING)
    }
}
