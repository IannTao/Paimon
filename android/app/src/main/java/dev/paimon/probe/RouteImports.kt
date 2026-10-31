package dev.paimon.probe

import android.content.Context
import android.util.AtomicFile
import android.util.Base64
import org.json.JSONObject
import java.io.File

/** One atomic envelope keeps the original bytes and provenance together, including across crashes. */
class RouteImports(context: Context) {
    private val latest = AtomicFile(File(context.filesDir, "route-import.json"))
    data class Saved(val name: String, val uri: String, val bytes: ByteArray)

    fun save(source: Saved, report: JSONObject) = synchronized(lock) {
        check(report.getString("status") != "invalid")
        val envelope = JSONObject().put("format", 1).put("name", source.name).put("uri", source.uri)
            .put("sha256", report.getString("sha256")).put("importedAt", System.currentTimeMillis())
            .put("upstreamCommit", report.opt("upstream_commit"))
            .put("bytes", Base64.encodeToString(source.bytes, Base64.NO_WRAP))
        val stream = latest.startWrite()
        try {
            stream.write(envelope.toString().toByteArray(Charsets.UTF_8))
            latest.finishWrite(stream)
        } catch (error: Throwable) { latest.failWrite(stream); throw error }
    }

    fun load(): Saved = synchronized(lock) {
        val encoded = latest.openRead().use { it.readNBytes(4 * 1024 * 1024 + 1) }
        require(encoded.size <= 4 * 1024 * 1024) { "保存文件过大" }
        val value = JSONObject(String(encoded, Charsets.UTF_8))
        require(value.getInt("format") == 1) { "保存格式不支持" }
        val bytes = Base64.decode(value.getString("bytes"), Base64.NO_WRAP)
        val saved = Saved(value.getString("name"), value.getString("uri"), bytes)
        val report = RouteNative.prepare(saved.name, bytes)
        check(report.getString("sha256") == value.getString("sha256")) { "保存文件身份校验失败" }
        saved
    }
    companion object { private val lock = Any() }
}
