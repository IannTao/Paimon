package dev.paimon.probe

import android.annotation.SuppressLint
import android.app.*
import android.content.Intent
import android.content.res.Configuration
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.os.IBinder
import android.provider.Settings
import android.view.*
import android.widget.Button
import kotlin.math.abs
import kotlin.math.roundToInt
import java.lang.ref.WeakReference

/** A navigation control independent of the bounded click experiment. */
class ReturnOverlayService : Service() {
    private lateinit var window: WindowManager
    private lateinit var button: Button
    private lateinit var params: WindowManager.LayoutParams

    // A short touch explicitly calls performClick; dragging must not open the app.
    @SuppressLint("ClickableViewAccessibility")
    override fun onCreate() {
        super.onCreate()
        val notifications = getSystemService(NotificationManager::class.java)
        notifications.createNotificationChannel(NotificationChannel("return", "返回 Paimon", NotificationManager.IMPORTANCE_LOW))
        val open = PendingIntent.getActivity(this, 0, appIntent(), PendingIntent.FLAG_IMMUTABLE or PendingIntent.FLAG_UPDATE_CURRENT)
        val close = PendingIntent.getService(this, 0, Intent(this, ReturnOverlayService::class.java).setAction("close"), PendingIntent.FLAG_IMMUTABLE)
        startForeground(2, Notification.Builder(this, "return").setSmallIcon(android.R.drawable.ic_menu_revert)
            .setContentTitle("返回 Paimon").setContentText("点击悬浮按钮返回；拖动可调整位置")
            .setContentIntent(open).addAction(Notification.Action.Builder(null, "关闭返回按钮", close).build()).build())
        if (!Settings.canDrawOverlays(this)) { stopSelf(); return }
        val display = checkNotNull(getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY))
        val context = createDisplayContext(display).createWindowContext(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY, null)
        window = context.getSystemService(WindowManager::class.java)
        val density = resources.displayMetrics.density
        params = WindowManager.LayoutParams(
            (80 * density).roundToInt(), (40 * density).roundToInt(),
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT
        ).apply { gravity = Gravity.TOP or Gravity.LEFT; x = 0; y = window.maximumWindowMetrics.bounds.height() / 2 }
        button = Button(ContextThemeWrapper(context, android.R.style.Theme_Material_Light)).apply {
            text = "Paimon"
            contentDescription = "返回 Paimon，拖动可移动"
            setOnClickListener {
                ProbeService.pauseForReturn()
                MovementService.stopForReturn()
                startActivity(appIntent())
            }
        }
        var downX = 0f
        var downY = 0f
        var originX = 0
        var originY = 0
        var dragging = false
        val slop = ViewConfiguration.get(context).scaledTouchSlop
        button.setOnTouchListener { view, event ->
            when (event.actionMasked) {
                MotionEvent.ACTION_DOWN -> {
                    downX = event.rawX; downY = event.rawY
                    originX = params.x; originY = params.y; dragging = false
                    view.isPressed = true
                }
                MotionEvent.ACTION_MOVE -> {
                    val dx = event.rawX - downX; val dy = event.rawY - downY
                    if (abs(dx) > slop || abs(dy) > slop) dragging = true
                    if (dragging) {
                        view.isPressed = false
                        params.x = originX + dx.roundToInt(); params.y = originY + dy.roundToInt()
                        keepOnScreen()
                    }
                }
                MotionEvent.ACTION_UP -> { view.isPressed = false; if (!dragging) view.performClick() }
                MotionEvent.ACTION_CANCEL -> { view.isPressed = false }
            }
            true
        }
        window.addView(button, params)
        active = WeakReference(this)
        keepOnScreen()
    }

    private fun appIntent() = Intent(this, MainActivity::class.java)
        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP)

    private fun keepOnScreen() {
        val bounds = window.maximumWindowMetrics.bounds
        params.x = params.x.coerceIn(0, (bounds.width() - params.width).coerceAtLeast(0))
        params.y = params.y.coerceIn(0, (bounds.height() - params.height).coerceAtLeast(0))
        window.updateViewLayout(button, params)
    }

    override fun onConfigurationChanged(newConfig: Configuration) {
        super.onConfigurationChanged(newConfig)
        if (::button.isInitialized && button.isAttachedToWindow) keepOnScreen()
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent?.action == "close") {
            getSharedPreferences("probe-form", MODE_PRIVATE).edit().putBoolean("returnOverlayEnabled", false).apply()
            stopSelf()
        }
        return START_NOT_STICKY
    }

    override fun onDestroy() {
        if (active.get() === this) active.clear()
        if (::button.isInitialized && button.isAttachedToWindow) window.removeView(button)
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null

    companion object {
        private var active = WeakReference<ReturnOverlayService>(null)

        fun containsPoint(x: Int, y: Int): Boolean {
            val view = active.get()?.button ?: return false
            val location = IntArray(2)
            view.getLocationOnScreen(location)
            return x >= location[0] && x < location[0] + view.width &&
                y >= location[1] && y < location[1] + view.height
        }
    }
}
