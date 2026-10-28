package dev.paimon.probe

import android.app.*
import android.content.*
import android.content.pm.PackageManager
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.os.*
import android.provider.Settings
import android.view.Gravity
import android.view.Display
import android.view.ContextThemeWrapper
import android.view.WindowManager
import android.widget.Button
import org.json.JSONArray
import org.json.JSONObject
import rikka.shizuku.Shizuku
import java.util.concurrent.Executors
import java.lang.ref.WeakReference
import java.io.FileDescriptor
import java.io.PrintWriter

class ProbeService : Service() {
    companion object {
        private var active = WeakReference<ProbeService>(null)

        // Both the overlay click and Activity lifecycle run on the main thread.
        fun pauseForReturn() { active.get()?.finishRun("Returned to Paimon") }
    }
    private val main = Handler(Looper.getMainLooper())
    private val worker = Executors.newSingleThreadExecutor()
    private var input: IProbeInput? = null
    private lateinit var window: WindowManager
    private lateinit var button: Button
    private lateinit var options: Intent
    private var bound = false
    @Volatile private var running = false
    private var pending = false
    @Volatile private var generation = 0
    @Volatile private var cancelling = false
    private var width = 0
    private var height = 0
    private var rotation = 0
    private var report: JSONObject? = null
    private var lastStopReason = "Not started"
    private var lastObservedWindow = ""
    private var attempts = JSONArray()
    private val args by lazy {
        Shizuku.UserServiceArgs(ComponentName(this, ProbeInputService::class.java))
            .daemon(false).processNameSuffix("input").version(2)
    }
    private val dead = Shizuku.OnBinderDeadListener { main.post { finishRun("Shizuku disconnected") } }
    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            input = IProbeInput.Stub.asInterface(binder)
            button.text = "已连接：点击开始\n5 秒后执行；再次点击停止"
        }
        override fun onServiceDisconnected(name: ComponentName?) {
            input = null
            finishRun("Input service disconnected")
        }
    }

    override fun onCreate() {
        super.onCreate()
        val primaryDisplay = checkNotNull(getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY))
        val overlayContext = createDisplayContext(primaryDisplay)
            .createWindowContext(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY, null)
        window = overlayContext.getSystemService(WindowManager::class.java)
        val notifications = getSystemService(NotificationManager::class.java)
        notifications.createNotificationChannel(NotificationChannel("probe", "触控验证", NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(this, 0, Intent(this, ProbeService::class.java).setAction("stop"), PendingIntent.FLAG_IMMUTABLE)
        startForeground(1, Notification.Builder(this, "probe").setSmallIcon(android.R.drawable.ic_media_pause)
            .setContentTitle("Paimon 触控验证").setContentText("有界点击实验；通过悬浮按钮或通知停止")
            .addAction(Notification.Action.Builder(null, "关闭验证", stop).build()).build())
        button = Button(ContextThemeWrapper(overlayContext, android.R.style.Theme_Material_Light)).apply {
            text = "等待 Shizuku 连接"
            textSize = 11f
            minHeight = 130
            setPadding(8, 8, 8, 8)
            setOnClickListener { if (running) finishRun("User stopped") else startRun() }
        }
        if (!Settings.canDrawOverlays(this)) { stopSelf(); return }
        val params = WindowManager.LayoutParams(
            360, WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT
        ).apply { gravity = Gravity.TOP or Gravity.LEFT; x = 0; y = 0 }
        window.addView(button, params)
        active = WeakReference(this)
        Shizuku.addBinderDeadListener(dead)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent == null || intent.action == "stop") { stopSelf(); return START_NOT_STICKY }
        if (!::options.isInitialized) options = Intent(intent)
        if (!bound) {
            try {
                check(Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED) { "Shizuku permission missing" }
                check(Shizuku.getUid() == 2000) { "Only shell-started Shizuku is supported" }
                Shizuku.bindUserService(args, connection)
                bound = true
            } catch (error: Exception) {
                button.text = "连接失败：${error.message}\n请关闭后重新授权"
            }
        }
        return START_NOT_STICKY
    }

    private fun startRun() {
        if (pending || cancelling) { button.text = "等待上次点击及停止操作结束"; return }
        val bridge = input ?: return
        val bounds = window.maximumWindowMetrics.bounds
        width = bounds.width(); height = bounds.height(); rotation = window.defaultDisplay.rotation
        val x = options.getIntExtra("x", -1)
        val y = options.getIntExtra("y", -1)
        val count = options.getIntExtra("count", 0)
        try {
            check(width > height) { "请先切换到约定的横屏方向" }
            check(ProbeAccessibilityService.connected) { "请启用无障碍服务" }
            check(bridge.foregroundPackage() == options.getStringExtra("gamePackage")) { "请先进入原神游戏画面（无法确认系统前台任务）" }
            lastObservedWindow = ProbeAccessibilityService.foregroundPackage
            check(bridge.uid() == 2000) { "输入进程不是 shell UID" }
            val location = IntArray(2)
            button.getLocationOnScreen(location)
            check(!(x >= location[0] && x < location[0] + button.width &&
                y >= location[1] && y < location[1] + button.height)) { "目标与左上角停止按钮重叠" }
            check(!ReturnOverlayService.containsPoint(x, y)) { "请将返回 Paimon 按钮拖离拾取位置" }
            check(ProbeNative.start(SystemClock.elapsedRealtime(), width, height, x, y, count)) { "坐标或次数无效（次数 2–30）" }
            bridge.begin()
            generation++
            attempts = JSONArray()
            report = JSONObject().put("schemaVersion", 1).put("runId", java.util.UUID.randomUUID().toString())
                .put("startedAt", java.time.Instant.now().toString())
                .put("manufacturer", Build.MANUFACTURER).put("model", Build.MODEL)
                .put("android", Build.VERSION.RELEASE).put("sdk", Build.VERSION.SDK_INT)
                .put("buildDisplay", Build.DISPLAY).put("fingerprint", Build.FINGERPRINT)
                .put("appVersion", BuildConfig.VERSION_NAME)
                .put("inputMethod", "Shizuku UserService shell /system/bin/input touchscreen tap")
                .put("inputUid", 2000).put("width", width).put("height", height).put("rotation", rotation)
                .put("foregroundCheck", "shell dumpsys resumed Activity; checked again before each tap")
                .put("x", x).put("y", y).put("requestedCount", count).put("countdownMs", 5000).put("intervalMs", 1000)
                .put("scenario", options.getStringExtra("scenario")).put("gameSettings", options.getStringExtra("settings"))
                .put("gamePackage", options.getStringExtra("gamePackage"))
                .put("attempts", attempts).put("gameAccepted", "unverified").put("itemObtained", "unverified")
                .put("touchContinuity", "unverified").put("overallGate", "unverified")
            running = true
            lastStopReason = ""
            ProbeReports.saveRun(this, checkNotNull(report), makeLatest = true)
            main.post(tick)
        } catch (error: Exception) {
            finishRun("未开始：${error.message}")
        }
    }

    private val tick = object : Runnable {
        override fun run() {
            if (!running) return
            try {
                check(Shizuku.pingBinder() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED) { "Shizuku unavailable" }
                check(ProbeAccessibilityService.connected) { "Accessibility disconnected" }
                val observedWindow = ProbeAccessibilityService.foregroundPackage
                if (observedWindow != lastObservedWindow) {
                    lastObservedWindow = observedWindow
                    check(input?.foregroundPackage() == options.getStringExtra("gamePackage")) { "Foreground game changed or unavailable" }
                }
                check(!(getSystemService(PowerManager::class.java).isInteractive.not() || getSystemService(KeyguardManager::class.java).isKeyguardLocked)) { "Screen locked" }
                check(!ReturnOverlayService.containsPoint(options.getIntExtra("x", -1), options.getIntExtra("y", -1))) { "返回按钮遮挡拾取位置" }
                val bounds = window.maximumWindowMetrics.bounds
                check(bounds.width() == width && bounds.height() == height && window.defaultDisplay.rotation == rotation) { "Display configuration changed" }
                when (val action = ProbeNative.next(SystemClock.elapsedRealtime())) {
                    -1L -> { finishRun("Bounded sequence ended"); return }
                    -2L -> { finishRun("Session deadline expired"); return }
                    0L -> Unit
                    else -> {
                        val point = action - 1
                        val x = (point ushr 32).toInt(); val y = point.toInt()
                        val token = generation
                        pending = true
                        val attempt = JSONObject().put("requestedAtElapsedMs", SystemClock.elapsedRealtime())
                            .put("x", x).put("y", y).put("commandResult", "pending")
                        attempts.put(attempt); saveReport()
                        worker.execute {
                            val result = runCatching {
                                if (running && token == generation) input?.tap(x, y, options.getStringExtra("gamePackage").orEmpty()) ?: -3 else -5
                            }
                            main.post {
                                pending = false
                                val code = result.getOrDefault(-4)
                                attempt.put("completedAtElapsedMs", SystemClock.elapsedRealtime()).put("exitCode", code)
                                    .put("commandResult", if (code == 0) "command-returned-zero-not-game-proof" else "failed")
                                result.exceptionOrNull()?.let { attempt.put("error", it.toString()) }
                                try {
                                    saveReport()
                                } catch (error: Exception) {
                                    finishRun("Report write failed: ${error.message}")
                                    return@post
                                }
                                if (token == generation && running) {
                                    ProbeNative.acknowledge(SystemClock.elapsedRealtime(), code == 0)
                                    if (code != 0) finishRun("Input command failed: $code")
                                }
                            }
                        }
                    }
                }
                button.text = "运行中（${attempts.length()} 次请求）\n点击立即停止后续点击"
                main.postDelayed(this, 50)
            } catch (error: Exception) { finishRun(error.message ?: error.toString()) }
        }
    }

    private fun finishRun(reason: String) {
        lastStopReason = reason
        val wasRunning = running
        running = false; generation++
        ProbeNative.stop()
        main.removeCallbacks(tick)
        if (wasRunning) {
            // Cancelling cannot retract a touch already accepted by Android.
            cancelling = true
            Thread { try { runCatching { input?.cancel() } } finally { cancelling = false } }.start()
            report?.put("endedAt", java.time.Instant.now().toString())?.put("stopReason", reason)
            runCatching { saveReport() }.onFailure {
                if (::button.isInitialized) button.text = "已停止；报告写入失败：${it.message}"
                return
            }
        }
        if (::button.isInitialized) button.text = "已停止：$reason\n点击可开启新一轮"
    }

    private fun saveReport() {
        val value = report ?: return
        ProbeReports.saveRun(this, value)
    }

    override fun onDestroy() {
        if (active.get() === this) active.clear()
        finishRun("Overlay service destroyed")
        Shizuku.removeBinderDeadListener(dead)
        if (bound) runCatching { Shizuku.unbindUserService(args, connection, true) }
        if (::button.isInitialized && button.isAttachedToWindow) window.removeView(button)
        worker.shutdownNow()
        super.onDestroy()
    }
    override fun onBind(intent: Intent?): IBinder? = null

    override fun dump(fd: FileDescriptor, writer: PrintWriter, args: Array<out String>) {
        writer.println("running=$running pending=$pending cancelling=$cancelling")
        writer.println("accessibilityConnected=${ProbeAccessibilityService.connected}")
        writer.println("observedWindowPackage=${ProbeAccessibilityService.foregroundPackage}")
        writer.println("resumedActivityPackage=${runCatching { input?.foregroundPackage() }.getOrNull()}")
        writer.println("inputConnected=${input != null} attempts=${attempts.length()}")
        writer.println("lastStopReason=$lastStopReason")
        writer.println("buttonText=${if (::button.isInitialized) button.text else "not-created"}")
    }
}
