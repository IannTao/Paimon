package dev.paimon.probe

import java.nio.ByteBuffer
import org.json.JSONObject

object RouteExecutionNative {
    init { System.loadLibrary("paimon_probe") }
    @JvmStatic private external fun plan(route: ByteBuffer, firstOnly: Boolean): ByteArray?
    @JvmStatic private external fun start(id: Long, route: ByteBuffer, firstOnly: Boolean, now: Long): ByteArray?
    @JvmStatic private external fun update(id: Long, now: Long, condition: Int, capturedAt: Long, mini: ByteBuffer?, screen: ByteBuffer?): ByteArray?
    @JvmStatic private external fun receipt(id: Long, serial: Long, now: Long, success: Boolean, released: Boolean): ByteArray?
    private fun direct(bytes: ByteArray) = ByteBuffer.allocateDirect(bytes.size).apply { put(bytes); rewind() }
    private fun decode(bytes: ByteArray?): JSONObject {
        val value = JSONObject(String(checkNotNull(bytes) { "路线核心会话失效" }, Charsets.UTF_8))
        check(!value.has("error")) { value.optString("error") }; return value
    }
    fun preview(route: ByteArray, firstOnly: Boolean = false) = decode(plan(direct(route), firstOnly))
    fun begin(id: Long, route: ByteArray, firstOnly: Boolean, now: Long) = decode(start(id, direct(route), firstOnly, now))
    fun step(id: Long, now: Long, condition: Int = 0, capturedAt: Long = 0, mini: ByteArray? = null, screen: ByteArray? = null) =
        decode(update(id, now, condition, capturedAt, mini?.let { direct(it) }, screen?.let { direct(it) }))
    fun acknowledge(id: Long, serial: Long, now: Long, success: Boolean, released: Boolean) = decode(receipt(id, serial, now, success, released))
}
