package dev.paimon.probe

import java.nio.ByteBuffer
import org.json.JSONObject

object RouteExecutionNative {
    init { System.loadLibrary("paimon_probe") }
    @JvmStatic private external fun plan(route: ByteBuffer, scope: Int): ByteArray?
    @JvmStatic private external fun start(id: Long, route: ByteBuffer, scope: Int, now: Long): ByteArray?
    @JvmStatic private external fun update(id: Long, now: Long, condition: Int, capturedAt: Long, mini: ByteBuffer?, screen: ByteBuffer?): ByteArray?
    @JvmStatic private external fun receipt(id: Long, serial: Long, now: Long, success: Boolean, released: Boolean): ByteArray?
    private fun direct(bytes: ByteArray) = ByteBuffer.allocateDirect(bytes.size).apply { put(bytes); rewind() }
    private fun decode(bytes: ByteArray?): JSONObject {
        val value = JSONObject(String(checkNotNull(bytes) { "路线核心会话失效" }, Charsets.UTF_8))
        check(!value.has("error")) { value.optString("error") }; return value
    }
    private fun scope(firstOnly: Boolean, firstPickup: Boolean): Int {
        require(!(firstOnly && firstPickup)) { "执行范围冲突" }
        return if (firstPickup) 2 else if (firstOnly) 1 else 0
    }
    fun preview(route: ByteArray, firstOnly: Boolean = false, firstPickup: Boolean = false) = decode(plan(direct(route), scope(firstOnly, firstPickup)))
    fun begin(id: Long, route: ByteArray, firstOnly: Boolean, now: Long, firstPickup: Boolean = false) = decode(start(id, direct(route), scope(firstOnly, firstPickup), now))
    fun step(id: Long, now: Long, condition: Int = 0, capturedAt: Long = 0, mini: ByteArray? = null, screen: ByteArray? = null) =
        decode(update(id, now, condition, capturedAt, mini?.let { direct(it) }, screen?.let { direct(it) }))
    fun acknowledge(id: Long, serial: Long, now: Long, success: Boolean, released: Boolean) = decode(receipt(id, serial, now, success, released))
}
