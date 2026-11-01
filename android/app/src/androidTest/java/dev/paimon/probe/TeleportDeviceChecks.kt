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
import android.widget.CheckBox
import android.widget.TextView
import org.json.JSONObject

/** Original route -> public start button -> real screen, real map taps, released result. */
class TeleportDeviceChecks : Instrumentation() {
    private var scenario = "teleport"
    override fun onCreate(arguments: Bundle?) { super.onCreate(arguments); scenario = arguments?.getString("scenario") ?: "teleport"; start() }
    override fun onStart() {
        val result = Bundle()
        try {
            val main = startActivitySync(Intent(targetContext, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
            awaitText(main, "UID 2000 / 授权 true")
            val monitor = addMonitor(RouteImportActivity::class.java.name, null, false)
            click(main, "导入/查看 BetterGI 路线")
            val route = checkNotNull(waitForMonitorWithTimeout(monitor, 5000)); removeMonitor(monitor)
            val filter = IntentFilter(Intent.ACTION_OPEN_DOCUMENT).apply { addCategory(Intent.CATEGORY_OPENABLE); addDataType("*/*") }
            val fixture = if (scenario == "unsupported") "force-tp" else "ground"
            val picker = addMonitor(filter, ActivityResult(Activity.RESULT_OK, Intent().setData(Uri.parse("content://dev.paimon.probe.test.routes/$fixture"))), true)
            try { click(route, "选择路线原文件"); check(picker.hits > 0) } finally { removeMonitor(picker) }
            awaitText(route, if (scenario == "unsupported") "路径点：1" else "路径点：7")
            if (scenario == "unsupported") {
                val previous = runCatching { NavigationReports.latest(targetContext).getLong("runId") }.getOrNull()
                click(route, "仅验证首个传送点（非整条通过）")
                awaitText(route, "传送目标、动作或参数尚未支持")
                check(runCatching { NavigationReports.latest(targetContext).getLong("runId") }.getOrNull() == previous)
                result.putString("result", "PASS unsupported force_tp: no input session started")
                finish(Activity.RESULT_OK, result); return
            }
            runOnMainSync { views(route.window.decorView).filterIsInstance<CheckBox>().single().isChecked = true }
            val started = SystemClock.elapsedRealtimeNanos()
            click(route, if (scenario == "whole") "启动整条受支持路线" else "仅验证首个传送点（非整条通过）")
            val until = SystemClock.uptimeMillis() + 195_000
            var proof: JSONObject? = null
            var triggered = false
            while (SystemClock.uptimeMillis() < until) {
                val report = runCatching { NavigationReports.latest(targetContext) }.getOrNull()
                if (report != null && report.optLong("runId") >= started) {
                    if (!triggered && scenario in listOf("stop", "foreground") && report.optLong("holdingSince") > 0) {
                        triggered = true
                        if (scenario == "stop") targetContext.startService(Intent(targetContext, NavigationService::class.java).setAction("stop"))
                        else targetContext.startActivity(Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                    }
                    if (!report.optBoolean("active") && report.optBoolean("releaseConfirmed")) { proof = report; break }
                }
                Thread.sleep(20)
            }
            val report = checkNotNull(proof) { "No released result: ${runCatching { NavigationReports.latest(targetContext) }.getOrNull()}" }
            val task = report.getJSONObject("payload").getJSONObject("task")
            when (scenario) {
                "stop" -> check(triggered && task.getString("state") == "stopped" && report.getBoolean("heldWhenCancelled")) { report.toString() }
                "foreground" -> check(triggered && task.getString("state") == "failed" && task.getString("reason").contains("前台")) { report.toString() }
                "wrong-start" -> check(task.getString("state") == "failed" && task.getInt("command_count") == 0 && !report.getBoolean("passed")) { report.toString() }
                else -> {
                    check(task.getString("state") == "completed" && report.getBoolean("passed")) { report.toString() }
                    check(task.getString("source_sha256") == "1646045d03cfb277e3651f341d0d876f9042b080e623149c217bb50517015dd2")
                    check(task.getBoolean("loading_observed") && task.getBoolean("teleport_validation_passed"))
                    val first = task.getJSONArray("completed_points").getJSONObject(0)
                    check(first.getInt("index") == 1 && first.getInt("original_id") == 1 && first.getString("point_type") == "teleport" && first.getDouble("distance") <= 8)
                    check(task.getBoolean("original_route_passed") == (scenario == "whole"))
                    if (scenario == "whole") check(task.getJSONArray("completed_points").length() == 7)
                }
            }
            if (scenario in listOf("stop", "foreground")) {
                Thread.sleep(1200)
                val after = NavigationReports.latest(targetContext)
                check(!after.getBoolean("active") && after.getJSONObject("payload").getJSONObject("task").getInt("command_count") == task.getInt("command_count"))
            }
            result.putString("result", "PASS $scenario: $report"); finish(Activity.RESULT_OK, result)
        } catch (error: Throwable) { result.putString("error", error.stackTraceToString()); finish(Activity.RESULT_CANCELED, result) }
    }
    private fun click(activity: Activity, label: String) = runOnMainSync { check(views(activity.window.decorView).filterIsInstance<Button>().single { it.text.toString() == label }.performClick()) }
    private fun awaitText(activity: Activity, expected: String) {
        val until = SystemClock.uptimeMillis() + 15000
        var actual = ""
        do {
            runOnMainSync { actual = views(activity.window.decorView).filterIsInstance<TextView>().joinToString("\n") { it.text } }
            if (actual.contains(expected)) return
            Thread.sleep(80)
        } while (SystemClock.uptimeMillis() < until)
        error("Missing $expected in $actual")
    }
    private fun views(view: View): List<View> = listOf(view) + if (view is ViewGroup) (0 until view.childCount).flatMap { views(view.getChildAt(it)) } else emptyList()
}
