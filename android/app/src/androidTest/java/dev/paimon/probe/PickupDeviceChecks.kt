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
class PickupDeviceChecks : Instrumentation() {
    private var scenario = "pickup"
    override fun onCreate(arguments: Bundle?) { super.onCreate(arguments); scenario = arguments?.getString("scenario") ?: "pickup"; start() }
    override fun onStart() {
        val result = Bundle()
        try {
            val main = startActivitySync(Intent(targetContext, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
            awaitActivityText(main, "UID 2000 / 授权 true")
            val monitor = addMonitor(RouteImportActivity::class.java.name, null, false)
            clickButton(main, "导入/查看 BetterGI 路线")
            val route = checkNotNull(waitForMonitorWithTimeout(monitor, 5000)); removeMonitor(monitor)
            val filter = IntentFilter(Intent.ACTION_OPEN_DOCUMENT).apply { addCategory(Intent.CATEGORY_OPENABLE); addDataType("*/*") }
            val fixture = if (scenario == "unsupported") "force-tp" else "ground"
            val picker = addMonitor(filter, ActivityResult(Activity.RESULT_OK, Intent().setData(Uri.parse("content://dev.paimon.probe.test.routes/$fixture"))), true)
            try { clickButton(route, "选择路线原文件"); check(picker.hits > 0) } finally { removeMonitor(picker) }
            awaitActivityText(route, if (scenario == "unsupported") "路径点：1" else "路径点：7")
            if (scenario == "unsupported") {
                val previous = runCatching { NavigationReports.latest(targetContext).getLong("runId") }.getOrNull()
                clickButton(route, "验证首个目标点拾取（非整条采集）")
                awaitActivityText(route, "传送目标、动作或参数尚未支持")
                check(runCatching { NavigationReports.latest(targetContext).getLong("runId") }.getOrNull() == previous)
                result.putString("result", "PASS unsupported force_tp: no input session started")
                finish(Activity.RESULT_OK, result); return
            }
            runOnMainSync { activityViews(route.window.decorView).filterIsInstance<CheckBox>().single().isChecked = true }
            val started = SystemClock.elapsedRealtimeNanos()
            clickButton(route, if (scenario == "target-7") "验证指定目标点拾取（非整条采集）" else "验证首个目标点拾取（非整条采集）")
            val until = SystemClock.uptimeMillis() + 195_000
            var proof: JSONObject? = null
            var triggered = false
            while (SystemClock.uptimeMillis() < until) {
                val report = runCatching { NavigationReports.latest(targetContext) }.getOrNull()
                if (report != null && report.optLong("runId") >= started) {
                    val payload = report.optJSONObject("payload")
                    // 导航交接也处于pickup_search，但可能带着上一手势的历史按住时间。
                    // 新turn命令会先清零该时间；这里只等待它被shell当前实际DOWN重新填入。
                    val searchingTurn = payload?.optJSONObject("task")?.optString("phase") == "pickup_search" &&
                        payload.optJSONObject("command")?.optString("kind") == "turn"
                    if (!triggered && scenario in listOf("stop", "foreground") && searchingTurn && report.optLong("holdingSince") > 0) {
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
                    check(task.getBoolean("pickup_validation_passed") && !task.getBoolean("original_route_passed"))
                    val pickup = task.getJSONObject("pickup_evidence")
                    val targetIndex = if (scenario == "target-7") 7 else 4
                    check(pickup.getInt("target_index") == targetIndex && pickup.getInt("original_id") == targetIndex && pickup.getString("item") == "琉璃百合" && pickup.getInt("quantity") == 1)
                    check(task.getInt("current_index") == targetIndex + 1 && pickup.getLong("obtained_captured_at") > pickup.getLong("prompt_captured_at"))
                    check(task.getJSONArray("completed_points").length() == targetIndex)
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
}
