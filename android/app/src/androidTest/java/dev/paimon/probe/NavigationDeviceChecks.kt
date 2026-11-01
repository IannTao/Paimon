package dev.paimon.probe

import android.app.Activity
import android.app.Instrumentation
import android.content.Intent
import android.content.IntentFilter
import android.content.ContextWrapper
import java.io.File
import org.json.JSONObject
import android.net.Uri
import android.os.Bundle
import android.os.SystemClock
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.CheckBox
import android.widget.TextView

/** Exercises the public import/start flow; the system document picker alone is substituted. */
class NavigationDeviceChecks : Instrumentation() {
    private var scenario = "segment"
    override fun onCreate(arguments: Bundle?) { super.onCreate(arguments); scenario = arguments?.getString("scenario") ?: "segment"; start() }
    override fun onStart() {
        val result = Bundle()
        try {
            checkReportOwnership()
            val main = startActivitySync(Intent(targetContext, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
            awaitText(main, "UID 2000 / 授权 true")
            val monitor = addMonitor(RouteImportActivity::class.java.name, null, false)
            click(main, "导入/查看 BetterGI 路线")
            val route = checkNotNull(waitForMonitorWithTimeout(monitor, 5000)); removeMonitor(monitor)
            val filter = IntentFilter(Intent.ACTION_OPEN_DOCUMENT).apply { addCategory(Intent.CATEGORY_OPENABLE); addDataType("*/*") }
            val picker = addMonitor(filter, ActivityResult(Activity.RESULT_OK, Intent().setData(Uri.parse("content://dev.paimon.probe.test.routes/${if (scenario == "orientation") "navigation" else "ground"}"))), true)
            try { click(route, "选择路线原文件"); check(picker.hits > 0) } finally { removeMonitor(picker) }
            awaitText(route, if (scenario == "orientation") "路径点：4" else "路径点：7")
            runOnMainSync { views(route.window.decorView).filterIsInstance<CheckBox>().single().isChecked = true }
            val startedAt = SystemClock.elapsedRealtimeNanos()
            click(route, if (scenario == "orientation") "启动整条受支持路线" else "仅验证导航分段（非原路线通过）")
            val deadline = SystemClock.uptimeMillis() + 135_000
            var triggered = false
            var proof: org.json.JSONObject? = null
            while (SystemClock.uptimeMillis() < deadline) {
                val report = runCatching { NavigationReports.latest(targetContext) }.getOrNull()
                if (report != null && report.optLong("runId") >= startedAt) {
                    if (!triggered && scenario in listOf("stop", "foreground") && report.optLong("holdingSince") > 0) {
                        triggered = true
                        if (scenario == "stop") targetContext.startService(Intent(targetContext, NavigationService::class.java).setAction("stop"))
                        else targetContext.startActivity(Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                    }
                    if (!report.optBoolean("active") && report.optBoolean("releaseConfirmed")) { proof = report; break }
                }
                Thread.sleep(25)
            }
            val report = checkNotNull(proof) { "No released result: ${runCatching { NavigationReports.latest(targetContext) }.getOrNull()}" }
            val task = report.getJSONObject("payload").getJSONObject("task")
            if (scenario in listOf("stop", "foreground")) {
                check(triggered && task.getString("state") != "completed") { report.toString() }
                if (scenario == "stop") check(task.getString("state") == "stopped" && report.getBoolean("heldWhenCancelled")) { report.toString() }
                else check(task.getString("state") == "failed" && task.getString("reason").contains("前台")) { report.toString() }
                Thread.sleep(1200)
                check(!NavigationReports.latest(targetContext).getBoolean("active"))
            } else {
                check(task.getString("state") == "completed") { report.toString() }
                check(task.getInt("move_count") > 0)
                if (scenario == "segment") {
                    check(task.getBoolean("segment_passed") && !task.getBoolean("original_route_passed"))
                    val points = task.getJSONArray("completed_points")
                    check(points.length() == 3 && (0..2).all { points.getJSONObject(it).getInt("index") == it + 2 })
                } else {
                    val points = task.getJSONArray("completed_points")
                    check(points.length() == 4 && points.getJSONObject(3).getString("point_type") == "orientation")
                    check(kotlin.math.abs(points.getJSONObject(3).getDouble("camera_error")) <= 12)
                    check((0..3).map { points.getJSONObject(it).getInt("original_id") } == listOf(2, 3, 4, 1))
                    val history = report.getJSONArray("history")
                    val orientationFrames = (0 until history.length()).map { history.getJSONObject(it) }
                        .filter { it.getJSONObject("task").getInt("current_index") == 4 }
                    check(orientationFrames.isNotEmpty() && orientationFrames.none { it.getJSONObject("command").getString("kind") == "move" })
                }
            }
            result.putString("result", "PASS $scenario: $report")
            finish(Activity.RESULT_OK, result)
        } catch (error: Throwable) { result.putString("error", error.stackTraceToString()); finish(Activity.RESULT_CANCELED, result) }
    }
    private fun checkReportOwnership() {
        val directory = File(targetContext.cacheDir, "report-check-${SystemClock.elapsedRealtimeNanos()}").apply { mkdirs() }
        val context = object : ContextWrapper(targetContext) { override fun getFilesDir() = directory }
        try {
            NavigationReports.claim(1)
            NavigationReports.save(context, JSONObject().put("runId", 1))
            NavigationReports.claim(2)
            NavigationReports.save(context, JSONObject().put("runId", 2))
            NavigationReports.save(context, JSONObject().put("runId", 1))
            check(NavigationReports.latest(context).getLong("runId") == 2L) { "A released old session overwrote a new session" }
        } finally { directory.deleteRecursively() }
    }
    private fun click(activity: Activity, label: String) = runOnMainSync {
        check(views(activity.window.decorView).filterIsInstance<Button>().single { it.text.toString() == label }.performClick())
    }
    private fun awaitText(activity: Activity, expected: String) {
        val until = SystemClock.uptimeMillis() + 15_000
        var text = ""
        do {
            runOnMainSync { text = views(activity.window.decorView).filterIsInstance<TextView>().joinToString("\n") { it.text } }
            if (text.contains(expected)) return
            Thread.sleep(80)
        } while (SystemClock.uptimeMillis() < until)
        error("Missing $expected in $text")
    }
    private fun views(view: View): List<View> = listOf(view) + if (view is ViewGroup)
        (0 until view.childCount).flatMap { views(view.getChildAt(it)) } else emptyList()
}
