package dev.paimon.probe

import android.app.*
import android.content.*
import android.content.pm.PackageManager
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.os.*
import android.provider.Settings
import android.view.*
import android.widget.Button
import org.json.JSONObject
import rikka.shizuku.Shizuku
import java.io.FileDescriptor
import java.io.PrintWriter
import java.lang.ref.WeakReference
import java.nio.ByteBuffer
import java.util.concurrent.Executors

/** Application entry for the single bounded, screen-observed movement task. */
class MovementService : Service() {
    private val main = Handler(Looper.getMainLooper())
    private val captures = Executors.newSingleThreadExecutor()
    private val moves = Executors.newSingleThreadExecutor()
    private lateinit var window: WindowManager
    private lateinit var button: Button
    private var bridge: IMovementInput? = null
    @Volatile private var running = false
    private var started = false
    private var bound = false
    private var busy = false
    @Volatile private var generation = 0
    private val id = SystemClock.elapsedRealtimeNanos()
    private var frameCount = 0
    private var lastFrameAt = 0L
    private var resultCode = 0
    private var releaseConfirmed: Boolean? = null
    private var detail = "等待输入服务"
    private var configuration = ""
    private var destroying = false
    private var cleanupStarted = false
    private var releaseFinished = false
    private var inputStartedAt = 0L
    private var inputReturnedAt = 0L
    private var releasedHoldAt = 0L
    private val deadline = Runnable { if (resultCode == 0) finish(MovementNative.TIMED_OUT, "应用侧任务期限已到") }
    private val args by lazy {
        Shizuku.UserServiceArgs(ComponentName(this, MovementInputService::class.java))
            .tag("movement-$id").daemon(false).processNameSuffix("movement").version(1)
    }
    private val dead = Shizuku.OnBinderDeadListener { main.post { finish(MovementNative.PERMISSION_LOST, "Shizuku disconnected") } }
    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            if (started) return
            bridge = IMovementInput.Stub.asInterface(binder)
            main.postDelayed({ startTask() }, 1000)
        }
        override fun onServiceDisconnected(name: ComponentName?) {
            bridge = null
            finish(MovementNative.PERMISSION_LOST, "输入服务断开")
        }
    }

    override fun onCreate() {
        super.onCreate()
        active = WeakReference(this)
        latestRunId = id
        val notifications = getSystemService(NotificationManager::class.java)
        notifications.createNotificationChannel(NotificationChannel("movement", "自动移动验证", NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(this, 7, Intent(this, MovementService::class.java).setAction("stop"), PendingIntent.FLAG_IMMUTABLE)
        startForeground(7, notification("自动移动验证", stop))
        val display = checkNotNull(getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY))
        val context = createDisplayContext(display).createWindowContext(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY, null)
        window = context.getSystemService(WindowManager::class.java)
        button = Button(ContextThemeWrapper(context, android.R.style.Theme_Material_Light)).apply {
            text = "07：准备自动移动\n点击停止"
            textSize = 11f
            setOnClickListener {
                if (resultCode == 0) finish(MovementNative.USER_STOPPED, "玩家主动停止")
                else {
                    startActivity(Intent(this@MovementService, MainActivity::class.java)
                        .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP))
                    stopSelf()
                }
            }
        }
        if (!Settings.canDrawOverlays(this)) { finish(MovementNative.PERMISSION_LOST, "缺少悬浮窗权限"); stopSelf(); return }
        window.addView(button, WindowManager.LayoutParams(
            600, WindowManager.LayoutParams.WRAP_CONTENT, WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT
        ).apply { gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL })
        Shizuku.addBinderDeadListener(dead)
        // Covers a service connection that never arrives as well as a stalled platform call.
        main.postDelayed(deadline, 12_000)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent == null || intent.action == "stop") {
            if (resultCode != 0) stopSelf() else finish(MovementNative.USER_STOPPED, "玩家主动停止")
            return START_NOT_STICKY
        }
        if (bound || resultCode != 0) return START_NOT_STICKY
        configuration = intent.getStringExtra("settings").orEmpty()
        saveResult()
        try {
            check(configuration.isNotBlank()) { "请记录固定游戏显示设置" }
            check(Shizuku.pingBinder() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
                && Shizuku.getUid() == 2000) { "需要 shell Shizuku 授权" }
            Shizuku.bindUserService(args, connection)
            bound = true
        } catch (error: Exception) { finish(MovementNative.PERMISSION_LOST, error.toString()) }
        return START_NOT_STICKY
    }

    private fun startTask() {
        if (started || resultCode != 0) return
        started = true
        running = true
        generation++
        MovementNative.start(SystemClock.elapsedRealtime())
        val token = generation
        val input = bridge ?: run { finish(MovementNative.PERMISSION_LOST, "输入服务不可用"); return }
        captures.execute {
            val preparation = runCatching {
                if (!running || token != generation) return@runCatching
                input.begin(id, GAME)
                if (!running || token != generation) input.cancel(id)
            }
            main.post {
                if (!running || token != generation) return@post
                if (preparation.isFailure) finish(MovementNative.PERMISSION_LOST, preparation.exceptionOrNull().toString())
                else main.post(tick)
            }
        }
    }

    private fun conditions(): Int = when {
        !Shizuku.pingBinder() || Shizuku.checkSelfPermission() != PackageManager.PERMISSION_GRANTED
            || !Settings.canDrawOverlays(this) -> 2
        !getSystemService(PowerManager::class.java).isInteractive || getSystemService(KeyguardManager::class.java).isKeyguardLocked -> 6
        window.maximumWindowMetrics.bounds.width() != 2480 || window.maximumWindowMetrics.bounds.height() != 1116
            || window.defaultDisplay.rotation != Surface.ROTATION_90 -> 5
        else -> 0
    }

    private val tick = object : Runnable {
        override fun run() {
            if (!running) return
            try {
                val condition = conditions()
                if (condition != 0) { finish(MovementNative.step(SystemClock.elapsedRealtime(), condition, 0, null), "运行条件改变"); return }
                val action = MovementNative.step(SystemClock.elapsedRealtime(), 0, 0, null)
                if (action >= MovementNative.MOVEMENT_CONFIRMED) { finish(action); return }
                if (!busy) capture()
                main.postDelayed(this, 50)
            } catch (error: Exception) { finish(MovementNative.PERMISSION_LOST, error.toString()) }
        }
    }

    private fun capture() {
        val input = bridge ?: run { finish(MovementNative.PERMISSION_LOST); return }
        val token = generation
        busy = true
        captures.execute {
            val captured = runCatching {
                check(running && token == generation)
                check(input.foregroundPackage() == GAME) { "原神不在前台" }
                input.capture(id)
            }
            val foreground = if (captured.isFailure) runCatching { input.foregroundPackage() }.getOrNull() else null
            val heldSince = if (captured.isSuccess) runCatching { input.holdingSince(id) }.getOrDefault(0L) else 0L
            main.post {
                if (!running || token != generation) return@post
                busy = false
                if (captured.isFailure) {
                    finish(if (!foreground.isNullOrEmpty() && foreground != GAME) MovementNative.WRONG_FOREGROUND else MovementNative.SCREEN_LOST,
                        captured.exceptionOrNull().toString())
                    return@post
                }
                try {
                    if (heldSince > 0 && inputStartedAt == 0L) {
                        inputStartedAt = heldSince
                        val stop = PendingIntent.getService(this@MovementService, 7,
                            Intent(this@MovementService, MovementService::class.java).setAction("stop"), PendingIntent.FLAG_IMMUTABLE)
                        getSystemService(NotificationManager::class.java).notify(7, notification("自动移动中", stop))
                        saveResult()
                    }
                    val packet = checkNotNull(captured.getOrNull())
                    check(packet.size == 8 + 496 * 224 * 3) { "画面包长度错误" }
                    val capturedAt = ByteBuffer.wrap(packet, 0, 8).long
                    val pixels = ByteBuffer.allocateDirect(packet.size - 8)
                    pixels.put(packet, 8, packet.size - 8).rewind()
                    frameCount++; lastFrameAt = capturedAt
                    val action = MovementNative.step(SystemClock.elapsedRealtime(), conditions(), capturedAt, pixels)
                    when {
                        action == 1 -> move(input, token)
                        action >= MovementNative.MOVEMENT_CONFIRMED -> finish(action)
                        else -> button.text = "07：${if (heldSince > 0) "自动移动中" else "观察移动反馈"}（$frameCount 帧）\n点击停止"
                    }
                } catch (error: Exception) { finish(MovementNative.SCREEN_LOST, error.toString()) }
            }
        }
    }

    private fun move(input: IMovementInput, token: Int) {
        button.text = "07：自动移动中\n点击停止"
        moves.execute {
            val moved = runCatching { running && token == generation && input.move(id) }
            val foreground = if (moved.getOrDefault(false)) null else runCatching { input.foregroundPackage() }.getOrNull()
            main.post {
                if (!running || token != generation) return@post
                val success = moved.getOrDefault(false)
                inputReturnedAt = SystemClock.uptimeMillis()
                MovementNative.inputFinished(SystemClock.elapsedRealtime(), success)
                if (!success) finish(if (!foreground.isNullOrEmpty() && foreground != GAME) MovementNative.WRONG_FOREGROUND else MovementNative.INPUT_FAILED,
                    moved.exceptionOrNull()?.toString().orEmpty())
            }
        }
    }

    private fun finish(code: Int, reason: String = "") {
        if (resultCode != 0) return
        resultCode = if (code >= MovementNative.MOVEMENT_CONFIRMED) code else MovementNative.SCREEN_LOST
        running = false; generation++
        main.removeCallbacks(tick)
        if (started) MovementNative.step(SystemClock.elapsedRealtime(), 1, 0, null)
        detail = reason
        button.text = MovementNative.describe(resultCode) + "\n正在释放输入"
        saveResult()
        releaseInput()
    }

    private fun releaseInput() {
        if (cleanupStarted) return
        cleanupStarted = true
        val input = bridge
        Thread {
            val releasedAt = runCatching { input?.cancel(id) ?: -1L }.getOrDefault(-1L)
            main.post {
                releaseConfirmed = releasedAt >= 0
                releasedHoldAt = releasedAt.coerceAtLeast(0)
                releaseFinished = true
                if (releasedAt < 0 && resultCode == MovementNative.MOVEMENT_CONFIRMED) resultCode = MovementNative.INPUT_FAILED
                button.text = MovementNative.describe(resultCode) + if (releasedAt >= 0) "\n输入已释放" else "\n输入释放未确认"
                saveResult()
                if (!destroying) {
                    val stop = PendingIntent.getService(this@MovementService, 7,
                        Intent(this@MovementService, MovementService::class.java).setAction("stop"), PendingIntent.FLAG_IMMUTABLE)
                    getSystemService(NotificationManager::class.java).notify(7, notification("移动验证已结束", stop))
                }
                if (destroying) disconnectInput()
            }
        }.start()
    }

    private fun notification(title: String, stop: PendingIntent): Notification =
        Notification.Builder(this, "movement").setSmallIcon(android.R.drawable.ic_media_pause)
            .setForegroundServiceBehavior(Notification.FOREGROUND_SERVICE_IMMEDIATE)
            .setContentTitle("Paimon $title")
            .setContentText(if (resultCode == 0) "点击悬浮按钮或通知停止" else MovementNative.describe(resultCode))
            .setContentIntent(PendingIntent.getActivity(this, 7, Intent(this, MainActivity::class.java)
                .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP), PendingIntent.FLAG_IMMUTABLE))
            .addExtras(Bundle().apply { putLong("movementRunId", id); putLong("holdingSinceUptimeMs", inputStartedAt) })
            .addAction(Notification.Action.Builder(null, "停止移动", stop).build()).build()

    private fun disconnectInput() {
        if (bound) {
            bound = false
            runCatching { Shizuku.unbindUserService(args, connection, true) }
        }
    }

    private fun saveResult() {
        runCatching {
            val result = JSONObject().put("runId", id).put("verdictCode", resultCode)
                .put("result", if (resultCode == 0) "运行中" else MovementNative.describe(resultCode)).put("detail", detail)
                .put("releaseConfirmed", releaseConfirmed ?: JSONObject.NULL)
                .put("frameCount", frameCount).put("lastFrameElapsedMs", lastFrameAt)
                .put("inputStartedUptimeMs", inputStartedAt).put("inputReturnedUptimeMs", inputReturnedAt)
                .put("releasedHoldAtUptimeMs", releasedHoldAt)
                .put("gamePackage", GAME).put("gameSettings", configuration)
                .put("inputPath", "Shizuku shell InputManager; bounded 900ms joystick; local 1200ms watchdog")
                .put("screenPath", "Shizuku shell screencap; memory only")
                .put("movementEvidence", if (resultCode == MovementNative.MOVEMENT_CONFIRMED) "stable-minimap-translation" else "unconfirmed")
                .put("overallGatheringGate", "unverified")
            // An old service can finish releasing after its replacement starts.
            // Keep that report, but never overwrite the newer run's latest result.
            MovementReports.save(this, result, latestRunId == id)
        }.onFailure { button.text = "任务已结束；报告保存失败：${it.message}" }
    }

    override fun onDestroy() {
        if (active.get() === this) active.clear()
        destroying = true
        finish(MovementNative.USER_STOPPED, "移动服务结束")
        main.removeCallbacks(tick)
        main.removeCallbacks(deadline)
        Shizuku.removeBinderDeadListener(dead)
        if (releaseFinished) disconnectInput()
        if (::button.isInitialized && button.isAttachedToWindow) window.removeView(button)
        captures.shutdownNow(); moves.shutdownNow()
        super.onDestroy()
    }

    override fun onBind(intent: Intent?): IBinder? = null
    override fun dump(fd: FileDescriptor, writer: PrintWriter, args: Array<out String>) {
        writer.println("running=$running started=$started resultCode=$resultCode frames=$frameCount releaseConfirmed=$releaseConfirmed")
        writer.println("detail=$detail")
    }

    companion object {
        const val GAME = "com.miHoYo.Yuanshen"
        private var active = WeakReference<MovementService>(null)
        private var latestRunId = 0L
        fun stopForReturn() { active.get()?.finish(MovementNative.USER_STOPPED, "返回 Paimon") }
    }
}
