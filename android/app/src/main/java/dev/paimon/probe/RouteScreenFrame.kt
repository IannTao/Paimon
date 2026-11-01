package dev.paimon.probe

import android.graphics.BitmapFactory
import java.nio.ByteBuffer

/** Bounded Binder packet: lossless original minimap plus a privacy-masked map overview. */
data class RouteScreenFrame(val capturedAt: Long, val mini: ByteArray, val screen: ByteArray?, val jpeg: ByteArray?) {
    companion object {
        fun decode(packet: ByteArray, full: Boolean): RouteScreenFrame {
            val miniEnd = 8 + 264 * 264 * 3
            check(packet.size >= miniEnd && packet.size <= miniEnd + 4 + 700 * 1024)
            val at = ByteBuffer.wrap(packet, 0, 8).long
            val mini = packet.copyOfRange(8, miniEnd)
            if (!full) { check(packet.size == miniEnd); return RouteScreenFrame(at, mini, null, null) }
            check(packet.size >= miniEnd + 4)
            val size = ByteBuffer.wrap(packet, miniEnd, 4).int
            check(size in 1..700 * 1024 && packet.size == miniEnd + 4 + size)
            val jpeg = packet.copyOfRange(miniEnd + 4, packet.size)
            val bounds = BitmapFactory.Options().apply { inJustDecodeBounds = true }
            BitmapFactory.decodeByteArray(jpeg, 0, jpeg.size, bounds)
            check(bounds.outWidth == 1240 && bounds.outHeight == 558)
            val bitmap = checkNotNull(BitmapFactory.decodeByteArray(jpeg, 0, jpeg.size))
            val colors = IntArray(1240 * 558)
            try { bitmap.getPixels(colors, 0, 1240, 0, 0, 1240, 558) } finally { bitmap.recycle() }
            val rgb = ByteArray(colors.size * 3)
            colors.forEachIndexed { n, c -> rgb[n * 3] = (c shr 16).toByte(); rgb[n * 3 + 1] = (c shr 8).toByte(); rgb[n * 3 + 2] = c.toByte() }
            return RouteScreenFrame(at, mini, rgb, jpeg)
        }
    }
}
