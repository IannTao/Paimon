package dev.paimon.probe

import java.nio.ByteBuffer

object MovementNative {
    const val MOVEMENT_CONFIRMED = 10
    const val NO_MOVEMENT_EVIDENCE = 11
    const val INVALID_SCREEN = 12
    const val USER_STOPPED = 13
    const val PERMISSION_LOST = 14
    const val SCREEN_LOST = 15
    const val WRONG_FOREGROUND = 16
    const val DISPLAY_CHANGED = 17
    const val LOCKED = 18
    const val TIMED_OUT = 19
    const val UNSTABLE_BASELINE = 20
    const val INPUT_FAILED = 21
    init { System.loadLibrary("paimon_probe") }
    @JvmStatic external fun start(now: Long)
    // 0: observe, 1: move once, 10+: terminal verdict (see describe).
    @JvmStatic external fun step(now: Long, condition: Int, capturedAt: Long, frame: ByteBuffer?): Int
    @JvmStatic external fun inputFinished(now: Long, success: Boolean)

    fun describe(code: Int): String = when (code) {
        MOVEMENT_CONFIRMED -> "小地图位移已确认（仅移动验证，不代表采集完成）"
        NO_MOVEMENT_EVIDENCE -> "未确认移动：缺少稳定的小地图位移证据"
        INVALID_SCREEN -> "不支持的游戏画面：请使用约定横屏、正常绿色血条和小地图布局"
        USER_STOPPED -> "已主动停止"
        PERMISSION_LOST -> "权限或输入服务失效"
        SCREEN_LOST -> "画面获取中断或过期"
        WRONG_FOREGROUND -> "原神不在前台"
        DISPLAY_CHANGED -> "屏幕配置改变"
        LOCKED -> "屏幕锁定"
        TIMED_OUT -> "任务超时"
        UNSTABLE_BASELINE -> "移动前小地图不稳定"
        INPUT_FAILED -> "输入失败"
        else -> "未验证：$code"
    }
}
