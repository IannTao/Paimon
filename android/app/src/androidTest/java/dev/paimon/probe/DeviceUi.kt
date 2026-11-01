package dev.paimon.probe

import android.app.Activity
import android.app.Instrumentation
import android.os.SystemClock
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.TextView

internal fun Instrumentation.clickButton(activity: Activity, label: String) = runOnMainSync {
    check(activityViews(activity.window.decorView).filterIsInstance<Button>().single { it.text.toString() == label }.performClick())
}

internal fun Instrumentation.awaitActivityText(activity: Activity, expected: String) {
    val until = SystemClock.uptimeMillis() + 15000
    var actual = ""
    do {
        runOnMainSync { actual = activityViews(activity.window.decorView).filterIsInstance<TextView>().joinToString("\n") { it.text } }
        if (actual.contains(expected)) return
        Thread.sleep(80)
    } while (SystemClock.uptimeMillis() < until)
    error("Missing $expected in $actual")
}

internal fun activityViews(view: View): List<View> = listOf(view) + if (view is ViewGroup)
    (0 until view.childCount).flatMap { activityViews(view.getChildAt(it)) } else emptyList()
