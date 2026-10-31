package dev.paimon.probe

import android.graphics.Bitmap
import android.graphics.BitmapFactory
import android.os.SystemClock
import org.json.JSONObject
import java.util.Locale

object LocalizationReplay {
    fun inspect(route: ByteArray, image: ByteArray): JSONObject {
        require(image.size <= 8 * 1024 * 1024) { "截图超过 8 MiB" }
        val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
        BitmapFactory.decodeByteArray(image, 0, image.size, bounds)
        require((bounds.outWidth == 2480 && bounds.outHeight == 1116) ||
            (bounds.outWidth == 264 && bounds.outHeight == 264)) { "只接受 2480×1116 游戏截图或 264×264 原尺寸小地图" }
        val bitmap = checkNotNull(BitmapFactory.decodeByteArray(image, 0, image.size)) { "PNG 读取失败" }
        val pixels = try { crop(bitmap) } finally { bitmap.recycle() }
        val id = SystemClock.elapsedRealtimeNanos()
        LocalizationNative.begin(id, route)
        val now = SystemClock.elapsedRealtime()
        return LocalizationNative.observe(id, now, now, pixels)
            .put("mode", "replay").put("navigation_allowed", false)
    }
    private fun crop(bitmap: Bitmap): ByteArray {
        val colors = IntArray(264 * 264)
        val x = if (bitmap.width == 264) 0 else 180
        val y = if (bitmap.height == 264) 0 else 28
        bitmap.getPixels(colors, 0, 264, x, y, 264, 264)
        return ByteArray(colors.size * 3).also { bytes ->
            colors.forEachIndexed { index, c -> bytes[index * 3] = (c shr 16).toByte(); bytes[index * 3 + 1] = (c shr 8).toByte(); bytes[index * 3 + 2] = c.toByte() }
        }
    }
    fun describe(report: JSONObject, historical: Boolean): String {
        val position = report.optJSONObject("position")
        val coordinates = if (position == null) "未定位" else String.format(Locale.ROOT, "%s：%.2f, %.2f",
            if (historical) "历史画面位置" else "当前位置", position.getDouble("x"), position.getDouble("y"))
        val heading = if (report.isNull("heading_degrees")) "未知" else String.format(Locale.ROOT, "%.1f°（北为0，顺时针）", report.getDouble("heading_degrees"))
        val target = report.optJSONObject("target")
        val trust = if (report.getString("state") == "located") "图像匹配已通过" else "未获得可信位置"
        return "$coordinates\n朝向：$heading\n目标点（第1个路径点）：${target ?: "无"}\n可信状态：$trust\n${report.getString("reason")}\n" +
            "地图参照：璃月港地表局部；其他地区/分层未支持\n" + if (historical) "非实时记录，禁止导航" else "只读定位准备，不发出移动输入"
    }
}
