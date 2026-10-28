package dev.paimon.probe

import android.accessibilityservice.AccessibilityService
import android.view.accessibility.AccessibilityEvent

class ProbeAccessibilityService : AccessibilityService() {
    override fun onServiceConnected() { connected = true }
    override fun onAccessibilityEvent(event: AccessibilityEvent?) {
        if (event?.eventType == AccessibilityEvent.TYPE_WINDOW_STATE_CHANGED) {
            foregroundPackage = event.packageName?.toString().orEmpty()
        }
    }
    override fun onInterrupt() { connected = false }
    override fun onDestroy() { connected = false; foregroundPackage = ""; super.onDestroy() }

    companion object {
        @Volatile var connected = false
            private set
        @Volatile var foregroundPackage = ""
            private set
    }
}
