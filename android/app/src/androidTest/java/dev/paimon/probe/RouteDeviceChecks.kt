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

/** Exercises the same picker result, ContentResolver, Rust, preview and offline reload as users. */
class RouteDeviceChecks : Instrumentation() {
    override fun onCreate(arguments: Bundle?) { super.onCreate(arguments); start() }
    override fun onStart() {
        val output = Bundle()
        try {
            progress("open main")
            val main = startActivitySync(Intent(targetContext, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))
            val routeMonitor = addMonitor(RouteImportActivity::class.java.name, null, false)
            click(main, "导入/查看 BetterGI 路线")
            val activity = checkNotNull(waitForMonitorWithTimeout(routeMonitor, 5000))
            removeMonitor(routeMonitor)
            fun import(name: String, vararg expected: String) {
                progress("import $name")
                val filter = IntentFilter(Intent.ACTION_OPEN_DOCUMENT).apply {
                    addCategory(Intent.CATEGORY_OPENABLE)
                    addDataType("*/*")
                }
                val picker = addMonitor(filter, ActivityResult(Activity.RESULT_OK,
                    Intent().setData(Uri.parse("content://dev.paimon.probe.test.routes/$name"))), true)
                try { click(activity, "选择路线原文件"); check(picker.hits > 0) { "Picker monitor did not intercept request" }; awaitText(activity, *expected) }
                finally { removeMonitor(picker) }
            }
            import("ground", "路径点：7", "267.96", "黎歌", "格式合法，执行能力未齐", "1646045d03cfb277", "路线🌸-ground.json")
            import("flight", "路径点：20", "stop_flying", "a2e2bb2ebc659f60")
            import("large", "能力：共 2004 项", "诊断：共 2004 项", "显示已截断")
            click(activity, "能力下一页")
            awaitText(activity, "config.key0050")
            check(!text(activity).contains("config.key1999")) { "All capabilities rendered at once" }
            import("many-errors", "格式错误", "诊断：共 2000 项")
            check(!text(activity).contains("positions[999]")) { "All errors rendered at once" }
            click(activity, "诊断下一页")
            awaitText(activity, "positions[25].x")
            import("jsonc", "路径点：2", "作者", "future", "config.new_behavior", "id=9", "id=1")
            import("invalid", "格式错误", "positions[0].x")
            check(!text(activity).contains("路径点：2")) { "Stale valid preview after failure" }
            click(activity, "读取已保存路线（离线）")
            awaitText(activity, "路径点：2", "自定义", "future")
            runOnMainSync {
                check(views(activity.window.decorView).filterIsInstance<Button>().none { it.isEnabled && it.text.contains("执行路线") })
                activity.finish(); main.finish()
            }
            output.putString("result", "PASS: original 7/20-point files, JSONC/defaults/metadata/duplicate ids, invalid input, offline reload, execution refused")
            finish(Activity.RESULT_OK, output)
        } catch (error: Throwable) {
            output.putString("error", error.stackTraceToString()); finish(Activity.RESULT_CANCELED, output)
        }
    }
    private fun progress(stage: String) = sendStatus(1, Bundle().apply { putString("stage", stage) })
    private fun click(activity: Activity, label: String) = runOnMainSync {
        check(views(activity.window.decorView).filterIsInstance<Button>().single { it.text.toString() == label }.performClick())
    }
    private fun text(activity: Activity): String {
        var value = ""
        runOnMainSync { value = views(activity.window.decorView).filterIsInstance<TextView>().joinToString("\n") { it.text } }
        return value
    }
    private fun awaitText(activity: Activity, vararg expected: String) {
        val until = SystemClock.uptimeMillis() + 8000
        do {
            val value = text(activity)
            if (expected.all { value.contains(it) }) return
            Thread.sleep(50)
        } while (SystemClock.uptimeMillis() < until)
        error("Missing ${expected.toList()} in ${text(activity)}")
    }
    private fun views(view: View): List<View> = listOf(view) + if (view is ViewGroup)
        (0 until view.childCount).flatMap { views(view.getChildAt(it)) } else emptyList()
}
