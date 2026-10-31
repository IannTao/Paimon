package dev.paimon.probe

import java.nio.ByteBuffer
import org.json.JSONObject

object NavigationNative {
    init { System.loadLibrary("paimon_probe") }
    @JvmStatic private external fun plan(route: ByteBuffer, first: Int, last: Int): ByteArray?
    @JvmStatic private external fun start(id: Long, route: ByteBuffer, first: Int, last: Int, now: Long): ByteArray?
    @JvmStatic private external fun update(id: Long, now: Long, condition: Int, capturedAt: Long, pixels: ByteBuffer?): ByteArray?
    @JvmStatic private external fun receipt(id: Long, serial: Long, now: Long, success: Boolean, released: Boolean): ByteArray?
    private fun direct(bytes: ByteArray) = ByteBuffer.allocateDirect(bytes.size).apply { put(bytes); rewind() }
    private fun decode(bytes: ByteArray?): JSONObject {
        val value = JSONObject(String(checkNotNull(bytes) { "导航核心会话已失效" }, Charsets.UTF_8))
        check(!value.has("error")) { value.optString("error") }
        return value
    }
    fun preview(route: ByteArray, first: Int = 0, last: Int = 0) = decode(plan(direct(route), first, last))
    fun begin(id: Long, route: ByteArray, first: Int, last: Int, now: Long) = decode(start(id, direct(route), first, last, now))
    fun step(id: Long, now: Long, condition: Int = 0, capturedAt: Long = 0, pixels: ByteArray? = null) =
        decode(update(id, now, condition, capturedAt, pixels?.let { direct(it) }))
    fun acknowledge(id: Long, serial: Long, now: Long, success: Boolean, released: Boolean) = decode(receipt(id, serial, now, success, released))
}
