package dev.paimon.probe

import java.nio.ByteBuffer
import org.json.JSONObject

object RouteExecutionNative {
    // JNI只传原文件和同次截图的像素；识别结果、到点及取得证据由Rust任务统一判定。
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
    private fun scope(firstOnly: Boolean, pickupMode: Boolean, pickupTargetIndex: Int): Int {
        // 显式区分两类局部验证，避免把“首点传送”和“首目标拾取”同时启用。
        require(!(firstOnly && pickupMode)) { "执行范围冲突" }
        require(pickupTargetIndex in 0..256 && (pickupMode || pickupTargetIndex == 0)) { "拾取目标范围无效" }
        return if (pickupMode) if (pickupTargetIndex > 0) pickupTargetIndex + 2 else 2 else if (firstOnly) 1 else 0
    }
    fun preview(route: ByteArray, firstOnly: Boolean = false, pickupMode: Boolean = false, pickupTargetIndex: Int = 0) = decode(plan(direct(route), scope(firstOnly, pickupMode, pickupTargetIndex)))
    fun begin(id: Long, route: ByteArray, firstOnly: Boolean, now: Long, pickupMode: Boolean = false, pickupTargetIndex: Int = 0) = decode(start(id, direct(route), scope(firstOnly, pickupMode, pickupTargetIndex), now))
    fun step(id: Long, now: Long, condition: Int = 0, capturedAt: Long = 0, mini: ByteArray? = null, screen: ByteArray? = null) =
        decode(update(id, now, condition, capturedAt, mini?.let { direct(it) }, screen?.let { direct(it) }))
    fun acknowledge(id: Long, serial: Long, now: Long, success: Boolean, released: Boolean) = decode(receipt(id, serial, now, success, released))
}
