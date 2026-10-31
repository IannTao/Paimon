package dev.paimon.probe

import java.nio.ByteBuffer
import org.json.JSONObject

object LocalizationNative {
    init { System.loadLibrary("paimon_probe") }
    @JvmStatic private external fun start(id: Long, route: ByteBuffer): ByteArray?
    @JvmStatic private external fun update(id: Long, now: Long, capturedAt: Long, pixels: ByteBuffer?): ByteArray?

    fun begin(id: Long, route: ByteArray): JSONObject = decode(start(id, direct(route)))
    fun observe(id: Long, now: Long, capturedAt: Long, pixels: ByteArray): JSONObject =
        decode(update(id, now, capturedAt, direct(pixels)))
    fun invalidate(id: Long, now: Long): JSONObject = decode(update(id, now, 0, null))
    private fun direct(bytes: ByteArray) = ByteBuffer.allocateDirect(bytes.size).apply { put(bytes); rewind() }
    private fun decode(bytes: ByteArray?) = JSONObject(String(checkNotNull(bytes) { "定位核心会话已失效" }, Charsets.UTF_8))
}
