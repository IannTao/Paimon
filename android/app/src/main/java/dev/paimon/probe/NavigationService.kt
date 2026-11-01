package dev.paimon.probe

import android.app.*
import android.content.*
import android.content.pm.PackageManager
import android.graphics.Bitmap
import android.graphics.Color
import android.graphics.PixelFormat
import android.hardware.display.DisplayManager
import android.os.*
import android.provider.Settings
import android.view.*
import android.widget.Button
import java.io.File
import java.lang.ref.WeakReference
import java.util.concurrent.Executors
import org.json.JSONArray
import org.json.JSONObject
import rikka.shizuku.Shizuku

/** Serializes core decisions; cancellation has its own executor, independent of capture/input. */
class NavigationService : Service() {
    private val main = Handler(Looper.getMainLooper())
    private val worker = Executors.newSingleThreadExecutor()
    private val cancelWorker = Executors.newSingleThreadExecutor()
    private val id = SystemClock.elapsedRealtimeNanos()
    @Volatile private var running = false
    private var ending = false
    private var bound = false
    private var busy = false
    private var busySince = 0L
    private var initialized = false
    private var releaseConfirmed = false
    private var cleanupReason = ""
    private var payload = JSONObject()
    private val history = JSONArray()
    private var historyBytes = 0
    private var historyTruncated = false
    private var reportingFailed = false
    private val receipts = JSONArray()
    private var frameCount = 0
    private var diagnostics = false
    private var holdingSince = 0L
    private var heldWhenCancelled = false
    private var bridge: INavigationInput? = null
    private var overlay: Button? = null
    private lateinit var window: WindowManager
    private var first = 0
    private var last = 0
    private var sha = ""
    private var routeExecution = false
    private var firstTeleportOnly = false
    private val args by lazy {
        Shizuku.UserServiceArgs(ComponentName(this, NavigationInputService::class.java))
            .tag("navigation-$id").daemon(false).processNameSuffix("navigation").version(1)
    }
    private val dead = Shizuku.OnBinderDeadListener { main.post { finish(2, "Shizuku 连接失效") } }
    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            if (!running || initialized) return
            val input = INavigationInput.Stub.asInterface(binder)
            bridge = input
            busy = true
            busySince = SystemClock.elapsedRealtime()
            worker.execute {
                val prepared = runCatching {
                    check(running)
                    val source = RouteImports(this@NavigationService).load()
                    check(RouteNative.prepare(source.name, source.bytes).getString("sha256") == sha) { "路线已改变" }
                    if (routeExecution) RouteExecutionNative.preview(source.bytes, firstTeleportOnly) else NavigationNative.preview(source.bytes, first, last)
                    input.begin(id)
                    if (routeExecution) RouteExecutionNative.begin(id, source.bytes, firstTeleportOnly, SystemClock.elapsedRealtime())
                    else NavigationNative.begin(id, source.bytes, first, last, SystemClock.elapsedRealtime())
                }
                main.post {
                    busy = false
                    if (!running) return@post
                    prepared.onSuccess { initialized = true; accept(it); main.postDelayed(tick, 150) }
                        .onFailure { finish(3, it.message ?: "准备失败") }
                }
            }
        }
        override fun onServiceDisconnected(name: ComponentName?) { finish(2, "输入服务断开") }
    }
    override fun onCreate() {
        super.onCreate()
        NavigationReports.claim(id)
        active = WeakReference(this)
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel("navigation", "路线执行", NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(this, 18, Intent(this, NavigationService::class.java).setAction("stop"), PendingIntent.FLAG_IMMUTABLE)
        startForeground(18, Notification.Builder(this, "navigation").setSmallIcon(android.R.drawable.ic_menu_mylocation)
            .setContentTitle("Paimon：自动路线执行").setContentText("点击停止并释放输入").setContentIntent(stop)
            .addAction(Notification.Action.Builder(null, "停止", stop).build()).build())
        val display = checkNotNull(getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY))
        val context = createDisplayContext(display).createWindowContext(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY, null)
        window = context.getSystemService(WindowManager::class.java)
        if (!Settings.canDrawOverlays(this)) { finish(2, "缺少悬浮窗权限"); return }
        overlay = Button(ContextThemeWrapper(context, android.R.style.Theme_Material_Light)).apply {
            text = "导航准备中 · 点击停止"; textSize = 11f
            setOnClickListener { finish(1, "已主动停止") }
        }
        window.addView(overlay, WindowManager.LayoutParams(820, WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT).apply { gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL })
        Shizuku.addBinderDeadListener(dead)
    }
    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent == null || intent.action == "stop") { finish(1, "已主动停止"); return START_NOT_STICKY }
        if (bound || ending) return START_NOT_STICKY
        first = intent.getIntExtra("first", 0); last = intent.getIntExtra("last", 0)
        routeExecution = intent.getBooleanExtra("routeExecution", false)
        firstTeleportOnly = intent.getBooleanExtra("firstTeleportOnly", false)
        main.postDelayed({ finish(3, "路线会话达到期限") }, if (routeExecution) 190_000 else 130_000)
        diagnostics = intent.getBooleanExtra("diagnostics", false)
        sha = intent.getStringExtra("routeSha").orEmpty()
        runCatching {
            check(sha.matches(Regex("[0-9a-f]{64}")))
            check(Shizuku.pingBinder() && Shizuku.getUid() == 2000 && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED)
            running = true
            if (!save()) return START_NOT_STICKY
            bound = true
            Shizuku.bindUserService(args, connection)
            main.postDelayed({ if (running && !initialized) finish(3, "初始化超时") }, 20_000)
        }.onFailure { finish(2, it.message ?: "不能启动") }
        return START_NOT_STICKY
    }
    private fun condition(): Int = when {
        !Shizuku.pingBinder() || Shizuku.checkSelfPermission() != PackageManager.PERMISSION_GRANTED || !Settings.canDrawOverlays(this) -> 2
        !getSystemService(PowerManager::class.java).isInteractive || getSystemService(KeyguardManager::class.java).isKeyguardLocked -> 6
        window.maximumWindowMetrics.bounds.width() != 2480 || window.maximumWindowMetrics.bounds.height() != 1116 -> 5
        getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY)?.rotation != Surface.ROTATION_90 -> 5
        else -> 0
    }
    private val tick = object : Runnable {
        override fun run() {
            if (!running) return
            val condition = condition()
            if (condition != 0) { finish(condition, "运行条件失效"); return }
            if (busy && SystemClock.elapsedRealtime() - busySince > 2300) { finish(3, "画面或输入操作超时"); return }
            if (!busy) capture()
            main.postDelayed(this, 50)
        }
    }
    private fun capture() {
        val input = bridge ?: return finish(2, "输入服务不可用")
        busy = true
        busySince = SystemClock.elapsedRealtime()
        val full = routeExecution && payload.optJSONObject("task")?.optString("phase") != "navigation"
        worker.execute {
            val result = runCatching {
                check(running)
                check(input.foregroundPackage() == MovementService.GAME) { "原神不在前台" }
                val frame = RouteScreenFrame.decode(if (full) input.captureScreen(id) else input.capture(id), full)
                if (diagnostics && frameCount < 160) {
                    saveFrame(frame.mini, frame.capturedAt)
                    if (frame.jpeg != null && frameCount < 64) File(filesDir, "navigation-frames/$id/${frame.capturedAt}-screen.jpg").writeBytes(frame.jpeg)
                }
                if (routeExecution) RouteExecutionNative.step(id, SystemClock.elapsedRealtime(), capturedAt = frame.capturedAt, mini = frame.mini, screen = frame.screen)
                else NavigationNative.step(id, SystemClock.elapsedRealtime(), capturedAt = frame.capturedAt, pixels = frame.mini)
            }
            main.post {
                if (!running) return@post
                busy = false
                result.onSuccess { frameCount++; accept(it) }.onFailure { finish(if (it.message?.contains("前台") == true) 4 else 3, it.message ?: "画面获取失败") }
            }
        }
    }
    private fun accept(value: JSONObject) {
        payload = value
        val record = JSONObject(value.toString()).put("observedAt", SystemClock.elapsedRealtime())
        record.getJSONObject("task").remove("completed_points")
        val size = record.toString().toByteArray(Charsets.UTF_8).size
        if (history.length() < 400 && historyBytes + size <= 512 * 1024) {
            history.put(record); historyBytes += size
        } else historyTruncated = true
        val task = value.getJSONObject("task")
        overlay?.text = "${if (firstTeleportOnly) "仅首点传送" else if (first == 0) "原路线" else "仅导航分段"} ${task.optInt("current_index")}/${task.optInt("last")} · ${task.optString("state")}\n${task.optString("reason")} · 点击停止"
        if (!save()) return
        when (value.getJSONObject("command").getString("kind")) {
            "end" -> finish(0, task.getString("reason"))
            "turn", "move", "tap" -> execute(value.getJSONObject("command"), task.getLong("captured_at"))
        }
    }
    private fun execute(command: JSONObject, at: Long) {
        val input = bridge ?: return finish(2, "输入服务不可用")
        val serial = command.getLong("serial")
        val turn = command.getString("kind") == "turn"
        val tap = command.getString("kind") == "tap"
        busy = true
        busySince = SystemClock.elapsedRealtime()
        worker.execute {
            val result = runCatching {
                check(running)
                val receipt = input.execute(id, serial, if (tap) 3 else if (turn) 1 else 2, command.optInt("dx"), if (tap) 80 else if (turn) 180 else command.getInt("duration_ms"), at, command.optInt("x"), command.optInt("y"))
                check(receipt.size == 4)
                receipt to if (routeExecution) RouteExecutionNative.acknowledge(id, serial, SystemClock.elapsedRealtime(), receipt[0] == 1L, receipt[1] == 1L)
                    else NavigationNative.acknowledge(id, serial, SystemClock.elapsedRealtime(), receipt[0] == 1L, receipt[1] == 1L)
            }.recoverCatching { error ->
                if (running && runCatching { input.foregroundPackage() != MovementService.GAME }.getOrDefault(false))
                    throw IllegalStateException("原神不在前台", error)
                throw error
            }
            main.post {
                if (!running) return@post
                result.onSuccess { (receipt, value) ->
                    if (receipts.length() < 400) receipts.put(JSONObject().put("serial", serial).put("kind", command.getString("kind"))
                        .put("success", receipt[0] == 1L).put("released", receipt[1] == 1L)
                        .put("downAt", receipt[2]).put("finishedAt", receipt[3]).put("command", command))
                    accept(value)
                    main.postDelayed({ if (running) busy = false }, if (tap) 40 else 180)
                }.onFailure { busy = false; finish(if (it.message?.contains("前台") == true) 4 else 3, it.message ?: "输入失败") }
            }
        }
        // A separate binder call observes a real held input; this never drives progress.
        cancelWorker.execute {
            repeat(8) {
                if (!running) return@execute
                val since = runCatching { input.holdingSince(id) }.getOrDefault(0)
                if (since > 0) { main.post { if (running) { holdingSince = since; save() } }; return@execute }
                Thread.sleep(25)
            }
        }
    }
    private fun saveFrame(rgb: ByteArray, at: Long) {
        val directory = File(filesDir, "navigation-frames/$id").apply { mkdirs() }
        val pixels = IntArray(264 * 264) { n -> Color.rgb(rgb[n * 3].toInt() and 255, rgb[n * 3 + 1].toInt() and 255, rgb[n * 3 + 2].toInt() and 255) }
        val bitmap = Bitmap.createBitmap(pixels, 264, 264, Bitmap.Config.ARGB_8888)
        try { File(directory, "$at.png").outputStream().use { bitmap.compress(Bitmap.CompressFormat.PNG, 100, it) } }
        finally { bitmap.recycle() }
    }
    private fun save(): Boolean {
        val result = runCatching { NavigationReports.save(this, JSONObject().put("runId", id).put("active", running).put("ending", ending)
            .put("routeSha256", sha).put("payload", payload).put("history", history).put("receipts", receipts)
            .put("frames", frameCount).put("releaseConfirmed", releaseConfirmed).put("cleanupReason", cleanupReason)
            .put("passed", !reportingFailed && releaseConfirmed && payload.optJSONObject("task")?.optString("state") == "completed")
            .put("historyTruncated", historyTruncated)
            .put("holdingSince", holdingSince).put("heldWhenCancelled", heldWhenCancelled).put("recordedAt", System.currentTimeMillis())) }
        if (result.isFailure) {
            reportingFailed = true
            cleanupReason = "导航报告保存失败：${result.exceptionOrNull()?.message}"
            if (!ending) finish(3, cleanupReason)
        }
        return result.isSuccess
    }
    private fun finish(condition: Int, reason: String) {
        if (ending) return
        ending = true; running = false; cleanupReason = reason
        main.removeCallbacks(tick)
        save()
        val input = bridge
        // Cancellation must never wait behind a blocked capture or input RPC.
        cancelWorker.execute {
            heldWhenCancelled = runCatching { input?.holdingSince(id)?.let { it > 0 } ?: false }.getOrDefault(false)
            val released = runCatching { input?.cancel(id) ?: !bound }.getOrDefault(false)
            val final = if (initialized && condition != 0) runCatching {
                if (routeExecution) RouteExecutionNative.step(id, SystemClock.elapsedRealtime(), condition)
                else NavigationNative.step(id, SystemClock.elapsedRealtime(), condition)
            }.getOrNull() else null
            main.post {
                releaseConfirmed = released
                if (final != null) payload = final
                save()
                overlay?.text = "$cleanupReason\n${if (!released) "未确认输入释放；任务未通过" else if (reportingFailed) "输入已释放；结果记录失败" else "输入已释放"}"
                if (bound) { bound = false; runCatching { Shizuku.unbindUserService(args, connection, true) } }
                bridge = null
                main.postDelayed({ stopSelf() }, 1500)
            }
        }
        main.postDelayed({
            if (!releaseConfirmed) {
                cleanupReason = "$reason；释放确认超时"; save()
                if (bound) { bound = false; runCatching { Shizuku.unbindUserService(args, connection, true) } }
                stopSelf()
            }
        }, 2500)
    }
    override fun onDestroy() {
        finish(1, "导航服务结束")
        Shizuku.removeBinderDeadListener(dead)
        overlay?.let { runCatching { window.removeView(it) } }; overlay = null
        worker.shutdownNow(); cancelWorker.shutdown()
        if (active.get() === this) active = WeakReference(null)
        super.onDestroy()
    }
    override fun onBind(intent: Intent?) = null
    companion object {
        private var active = WeakReference<NavigationService>(null)
        fun stopForReturn() { active.get()?.let { service -> service.main.post { service.finish(1, "已返回 Paimon") } } }
    }
}
