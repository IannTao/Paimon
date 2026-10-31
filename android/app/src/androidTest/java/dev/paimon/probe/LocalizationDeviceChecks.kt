package dev.paimon.probe

import android.app.Activity
import android.app.Instrumentation
import android.content.Intent
import android.content.IntentFilter
import android.net.Uri
import android.os.Bundle
import android.os.SystemClock
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.TextView

/** Starts at the route preparation UI; only the external file chooser is intercepted. */
class LocalizationDeviceChecks : Instrumentation() {
    private var live = false
    private var foregroundLoss = false
    private var reference = 127
    override fun onCreate(arguments: Bundle?) {
        super.onCreate(arguments)
        live = arguments?.getString("scenario")?.startsWith("live") == true
        foregroundLoss = arguments?.getString("scenario") == "live-foreground"
        reference = arguments?.getString("reference")?.toInt() ?: 127
        start()
    }
    override fun onStart() {
        val result = Bundle()
        try {
            val main = startActivitySync(Intent(targetContext, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
            if (live) awaitText(main, "UID 2000 / 授权 true")
            val monitor = addMonitor(RouteImportActivity::class.java.name, null, false)
            click(main, "导入/查看 BetterGI 路线")
            val route = checkNotNull(waitForMonitorWithTimeout(monitor, 5000)); removeMonitor(monitor)
            choose(route, "选择路线原文件", "*/*", "ground")
            awaitText(route, "路径点：7")
            if (live) {
                val startedAt = SystemClock.elapsedRealtimeNanos()
                click(route, "开始实时定位准备（进入原神）")
                val preparation = text(route)
                val deadline = SystemClock.uptimeMillis() + 20_000
                var proof: String? = null
                while (SystemClock.uptimeMillis() < deadline) {
                    val report = runCatching { LocalizationReports.latest(targetContext) }.getOrNull()
                    val observed = report?.optJSONObject("observation")
                    if (report != null && report.optLong("runId") >= startedAt && report.optBoolean("active") && report.optInt("frames") >= 2 &&
                        observed?.optString("state") == "located" && observed.optLong("valid_until") >= SystemClock.elapsedRealtime()) {
                        val p = observed.getJSONObject("position")
                        val expected = if (reference == 128) 267.9441 to -665.0128 else 366.71155 to -502.4061
                        check(kotlin.math.hypot(p.getDouble("x") - expected.first, p.getDouble("y") - expected.second) < 8.0)
                        check(!observed.getBoolean("navigation_allowed") && report.getInt("inputOperations") == 0)
                        proof = report.toString(); break
                    }
                    Thread.sleep(100)
                }
                checkNotNull(proof) { "No live screen-derived fix: ${runCatching { LocalizationReports.latest(targetContext) }.fold({it.toString()}, {it.stackTraceToString()})}; preparation=$preparation" }
                if (foregroundLoss) targetContext.startActivity(Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                else targetContext.startService(Intent(targetContext, LocalizationService::class.java).setAction("stop"))
                val untilStopped = SystemClock.uptimeMillis() + 5000
                var stopped = false
                do {
                    val after = LocalizationReports.latest(targetContext)
                    val observed = after.getJSONObject("observation")
                    stopped = !after.getBoolean("active") && observed.isNull("position") && observed.isNull("heading_degrees")
                    if (stopped) {
                        if (foregroundLoss) check(observed.getString("reason").contains("原神不在前台"))
                        break
                    }
                    Thread.sleep(100)
                } while (SystemClock.uptimeMillis() < untilStopped)
                check(stopped) { "Stopping did not invalidate current coordinates" }
                result.putString("result", "PASS live: $proof")
            } else {
                choose(route, "用截图检查定位（历史画面）", "image/png", "image-128")
                awaitText(route, "历史画面位置：267.27", "-664.55", "朝向：93", "非实时")
                choose(route, "用截图检查定位（历史画面）", "image/png", "image-127")
                awaitText(route, "历史画面位置：365.98", "-501.91", "朝向：76")
                choose(route, "用截图检查定位（历史画面）", "image/png", "image-chasm")
                awaitText(route, "未定位", "没有可信地图匹配")
                check(!text(route).contains("历史画面位置：365.98")) { "Previous fix survived image failure" }
                result.putString("result", "PASS replay: real images at two references, heading, underground rejection; historical positions cannot start navigation")
                runOnMainSync { route.finish(); main.finish() }
            }
            finish(Activity.RESULT_OK, result)
        } catch (error: Throwable) { result.putString("error", error.stackTraceToString()); finish(Activity.RESULT_CANCELED, result) }
    }
    private fun choose(activity: Activity, label: String, type: String, fixture: String) {
        val filter = IntentFilter(Intent.ACTION_OPEN_DOCUMENT).apply { addCategory(Intent.CATEGORY_OPENABLE); addDataType(type) }
        val picker = addMonitor(filter, ActivityResult(Activity.RESULT_OK, Intent().setData(Uri.parse("content://dev.paimon.probe.test.routes/$fixture"))), true)
        try { click(activity, label); check(picker.hits > 0) } finally { removeMonitor(picker) }
    }
    private fun click(activity: Activity, label: String) = runOnMainSync {
        check(views(activity.window.decorView).filterIsInstance<Button>().single { it.text.toString() == label }.performClick())
    }
    private fun text(activity: Activity): String {
        var value = ""
        runOnMainSync { value = views(activity.window.decorView).filterIsInstance<TextView>().joinToString("\n") { it.text } }
        return value
    }
    private fun awaitText(activity: Activity, vararg expected: String) {
        val deadline = SystemClock.uptimeMillis() + 12_000
        do { if (expected.all { text(activity).contains(it) }) return; Thread.sleep(80) } while (SystemClock.uptimeMillis() < deadline)
        error("Missing ${expected.toList()} in ${text(activity)}")
    }
    private fun views(view: View): List<View> = listOf(view) + if (view is ViewGroup)
        (0 until view.childCount).flatMap { views(view.getChildAt(it)) } else emptyList()
}
