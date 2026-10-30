package dev.paimon.probe

import android.app.Activity
import android.app.Instrumentation
import android.content.Intent
import android.os.Bundle
import android.os.SystemClock
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.EditText
import android.widget.TextView
import org.json.JSONObject

/** Opt-in real-device check through the application start and result controls.
 * Requires the baseline game already prepared; never grants permissions itself.
 * Does not use UiAutomation, which can suppress accessibility services.
 */
class MovementDeviceChecks : Instrumentation() {
    private lateinit var arguments: Bundle

    override fun onCreate(arguments: Bundle?) {
        super.onCreate(arguments)
        this.arguments = arguments ?: Bundle()
        start()
    }

    override fun onStart() {
        val output = Bundle()
        try {
            val settings = checkNotNull(arguments.getString("settings")) { "Pass the actual scene/settings record" }
            val scenario = arguments.getString("scenario", "movement")
            require(scenario in listOf("movement", "stop", "foreground"))
            val activity = openApp()
            val testStartedAt = SystemClock.elapsedRealtimeNanos()
            // Allow the provider to reconnect Shizuku after instrumentation
            // starts the application process. Observe the existing status UI.
            var ready = false
            var preparation = ""
            for (attempt in 0 until 25) {
                runOnMainSync {
                    preparation = views(activity.window.decorView).filterIsInstance<TextView>()
                        .map { it.text.toString() }.firstOrNull { it.contains("Shizuku：") }.orEmpty()
                    ready = preparation.contains("UID 2000 / 授权 true")
                }
                if (ready) break
                Thread.sleep(200)
            }
            check(ready) { "Preparation not ready: $preparation" }
            runOnMainSync {
                val views = views(activity.window.decorView)
                views.filterIsInstance<EditText>().single { it.hint.toString().startsWith("07 ") }.setText(settings)
                views.filterIsInstance<Button>().single { it.text.toString().startsWith("启动 07 ") }.performClick()
            }
            if (scenario != "movement") {
                var interrupted = false
                for (attempt in 0 until 350) {
                    val progress = runCatching { MovementReports.latest(targetContext) }.getOrNull()
                    if (progress != null && progress.getLong("runId") >= testStartedAt && progress.getInt("verdictCode") == 0 &&
                        progress.getLong("inputStartedUptimeMs") > 0 &&
                        SystemClock.uptimeMillis() - progress.getLong("inputStartedUptimeMs") < 850) {
                        if (scenario == "stop") {
                            // The same task-stop command used by the notification;
                            // notification permission remains optional for users.
                            targetContext.startService(Intent(targetContext, MovementService::class.java).setAction("stop"))
                        }
                        else targetContext.startActivity(Intent(Intent.ACTION_MAIN).addCategory(Intent.CATEGORY_HOME)
                            .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK))
                        interrupted = true
                        break
                    }
                    Thread.sleep(20)
                }
                check(interrupted) { "No live held-input phase was observed; interruption was not tested" }
            }
            // The application has an independent 12s deadline; this is a smoke
            // check, not a scheduler or a replacement for the phone-local bounds.
            Thread.sleep(12_500)
            val resultActivity = openApp()
            var result = ""
            runOnMainSync {
                val views = views(resultActivity.window.decorView)
                views.filterIsInstance<Button>().single { it.text.toString() == "查看 07 最新结果" }.performClick()
                result = views.filterIsInstance<TextView>().map { it.text.toString() }
                    .firstOrNull { it.startsWith("{") && it.contains("verdictCode") }.orEmpty()
            }
            val report = JSONObject(result)
            check(report.getLong("runId") >= testStartedAt) { "No new task result (old report): $report" }
            check(report.getBoolean("releaseConfirmed")) { "Input release not confirmed: $report" }
            if (scenario == "movement") check(report.getInt("verdictCode") == MovementNative.MOVEMENT_CONFIRMED) {
                "Movement not proven: $report"
            }
            else check(report.getInt("verdictCode") == if (scenario == "stop") MovementNative.USER_STOPPED else MovementNative.WRONG_FOREGROUND) {
                "Wrong interruption result: $report"
            }
            if (scenario == "stop") check(report.getLong("releasedHoldAtUptimeMs") > report.getLong("inputStartedUptimeMs")) {
                "Stop did not cancel a held joystick: $report"
            }
            output.putString("report", report.toString())
            finish(Activity.RESULT_OK, output)
        } catch (error: Throwable) {
            output.putString("error", error.toString())
            finish(Activity.RESULT_CANCELED, output)
        }
    }

    private fun openApp(): Activity = startActivitySync(Intent(targetContext, MainActivity::class.java)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TASK))

    private fun views(root: View): List<View> = buildList {
        add(root)
        if (root is ViewGroup) for (index in 0 until root.childCount) addAll(views(root.getChildAt(index)))
    }
}
