package dev.paimon.probe

import java.nio.ByteBuffer
import org.json.JSONObject

/** Read-only route preparation; deliberately independent of movement/input services. */
object RouteNative {
    const val MAX_BYTES = 2 * 1024 * 1024
    init { System.loadLibrary("paimon_probe") }
    @JvmStatic private external fun inspect(data: ByteBuffer, name: ByteBuffer): ByteArray?

    fun prepare(name: String, bytes: ByteArray): JSONObject {
        require(bytes.size <= MAX_BYTES) { "文件超过 2 MiB" }
        val encodedName = name.toByteArray(Charsets.UTF_8)
        require(encodedName.size in 1..4096) { "文件名长度无效" }
        fun direct(bytes: ByteArray) = ByteBuffer.allocateDirect(bytes.size).apply { put(bytes); rewind() }
        val result = checkNotNull(inspect(direct(bytes), direct(encodedName))) { "路线预检失败" }
        return JSONObject(String(result, Charsets.UTF_8))
    }
}
