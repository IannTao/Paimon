package dev.paimon.probe

object ProbeNative {
    init { System.loadLibrary("paimon_probe") }
    @JvmStatic external fun start(now: Long, width: Int, height: Int, x: Int, y: Int, count: Int): Boolean
    // 0: waiting, -1: finished/stopped, -2: expired, positive: packed (x,y) + 1.
    @JvmStatic external fun next(now: Long): Long
    @JvmStatic external fun acknowledge(now: Long, success: Boolean)
    @JvmStatic external fun stop()
}
