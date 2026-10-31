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
import java.lang.ref.WeakReference
import java.nio.ByteBuffer
import java.util.concurrent.Executors

/** A bounded observation session. This service has no movement or teleport operation. */
class LocalizationService : Service() {
    private val main = Handler(Looper.getMainLooper())
    private val worker = Executors.newSingleThreadExecutor()
    private val id = SystemClock.elapsedRealtimeNanos()
    private lateinit var window: WindowManager
    private var overlay: Button? = null
    private var bridge: ILocalizationScreen? = null
    private var bound = false
    private var busy = false
    private var initializing = false
    @Volatile private var running = false
    private var ended = false
    private var frames = 0
    private var observation = emptyObservation("等待真实画面")
    private var lastFix: JSONObject? = null
    private var routeSha = ""
    private var routeName = ""
    private val args by lazy {
        Shizuku.UserServiceArgs(ComponentName(this, LocalizationScreenService::class.java))
            .tag("localization-$id").daemon(false).processNameSuffix("localization").version(1)
    }
    private val dead = Shizuku.OnBinderDeadListener { main.post { finish("Shizuku 连接失效") } }
    private val connection = object : ServiceConnection {
        override fun onServiceConnected(name: ComponentName?, binder: IBinder?) {
            if (!running || initializing) return
            initializing = true
            bridge = ILocalizationScreen.Stub.asInterface(binder)
            worker.execute {
                val prepared = runCatching {
                    val source = RouteImports(this@LocalizationService).load()
                    val report = RouteNative.prepare(source.name, source.bytes)
                    check(report.getString("sha256") == routeSha) { "选择的路线已改变，请重新开始" }
                    routeName = report.getString("name")
                    check(report.getString("map_name") == "Teyvat" && report.getString("status") != "invalid") { "路线地图不支持或数据无效" }
                    val capabilities = report.getJSONArray("capabilities")
                    check((0 until capabilities.length()).any {
                        val capability = capabilities.getJSONObject(it)
                        capability.getString("field") == "world_localization" && capability.getBoolean("executable")
                    }) { "未验证此路线的地图范围或定位配置" }
                    LocalizationNative.begin(id, source.bytes)
                }
                main.post {
                    if (!running) return@post
                    prepared.onSuccess { observation = it; save(); main.postDelayed(tick, 750) }
                        .onFailure { finish(it.message ?: "定位核心准备失败") }
                }
            }
        }
        override fun onServiceDisconnected(name: ComponentName?) { bridge = null; finish("截图服务断开") }
    }

    override fun onCreate() {
        super.onCreate()
        active = WeakReference(this)
        getSystemService(NotificationManager::class.java).createNotificationChannel(
            NotificationChannel("localization", "屏幕定位准备", NotificationManager.IMPORTANCE_LOW))
        val stop = PendingIntent.getService(this, 17, Intent(this, LocalizationService::class.java).setAction("stop"), PendingIntent.FLAG_IMMUTABLE)
        val notification = Notification.Builder(this, "localization").setSmallIcon(android.R.drawable.ic_menu_mylocation)
            .setContentTitle("Paimon：只读定位准备").setContentText("不移动角色；点此停止")
            .setContentIntent(stop).addAction(Notification.Action.Builder(null, "停止定位", stop).build()).build()
        startForeground(17, notification)
        val display = checkNotNull(getSystemService(DisplayManager::class.java).getDisplay(Display.DEFAULT_DISPLAY))
        val context = createDisplayContext(display).createWindowContext(WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY, null)
        window = context.getSystemService(WindowManager::class.java)
        if (!Settings.canDrawOverlays(this)) { finish("缺少悬浮窗权限"); stopSelf(); return }
        overlay = Button(ContextThemeWrapper(context, android.R.style.Theme_Material_Light)).apply {
            text = "定位准备中；不操作游戏\n点击停止"; textSize = 11f
            setOnClickListener {
                finish("已主动停止定位准备")
                startActivity(Intent(this@LocalizationService, RouteImportActivity::class.java)
                    .addFlags(Intent.FLAG_ACTIVITY_NEW_TASK or Intent.FLAG_ACTIVITY_CLEAR_TOP))
                stopSelf()
            }
        }
        window.addView(overlay, WindowManager.LayoutParams(820, WindowManager.LayoutParams.WRAP_CONTENT,
            WindowManager.LayoutParams.TYPE_APPLICATION_OVERLAY,
            WindowManager.LayoutParams.FLAG_NOT_FOCUSABLE or WindowManager.LayoutParams.FLAG_NOT_TOUCH_MODAL,
            PixelFormat.TRANSLUCENT).apply { gravity = Gravity.TOP or Gravity.CENTER_HORIZONTAL })
        Shizuku.addBinderDeadListener(dead)
        main.postDelayed({ finish("定位准备达到 45 秒期限") }, 45_000)
    }

    override fun onStartCommand(intent: Intent?, flags: Int, startId: Int): Int {
        if (intent == null || intent.action == "stop") { finish("已主动停止定位准备"); return START_NOT_STICKY }
        if (bound || ended) return START_NOT_STICKY
        routeSha = intent.getStringExtra("routeSha").orEmpty()
        try {
            check(routeSha.matches(Regex("[0-9a-f]{64}"))) { "缺少选定路线身份" }
            check(Shizuku.pingBinder() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED && Shizuku.getUid() == 2000) { "需要 shell Shizuku 授权" }
            running = true
            save()
            bound = true
            Shizuku.bindUserService(args, connection)
            main.postDelayed({ if (running && frames == 0) finish("截图或核心初始化超时") }, 20_000)
        } catch (error: Exception) { finish(error.message ?: "无法开始定位准备") }
        return START_NOT_STICKY
    }

    private val tick = object : Runnable {
        override fun run() {
            if (!running) return
            val reason = when {
                !Shizuku.pingBinder() || Shizuku.checkSelfPermission() != PackageManager.PERMISSION_GRANTED -> "截图权限失效"
                !Settings.canDrawOverlays(this@LocalizationService) -> "悬浮窗权限失效"
                !getSystemService(PowerManager::class.java).isInteractive || getSystemService(KeyguardManager::class.java).isKeyguardLocked -> "屏幕已锁定"
                window.maximumWindowMetrics.bounds.width() != 2480 || window.maximumWindowMetrics.bounds.height() != 1116 -> "屏幕布局改变"
                else -> null
            }
            if (reason != null) { finish(reason); return }
            val now = SystemClock.elapsedRealtime()
            if (!observation.isNull("valid_until") && now > observation.getLong("valid_until")) {
                clearObservation("画面已过期，不能导航"); render(); save()
            }
            if (!busy) capture()
            main.postDelayed(this, 200)
        }
    }

    private fun capture() {
        val screen = bridge ?: run { finish("截图服务不可用"); return }
        busy = true
        worker.execute {
            val result = runCatching {
                check(running)
                val packet = screen.capture()
                check(packet.size == 8 + 264 * 264 * 3) { "小地图画面包长度错误" }
                val capturedAt = ByteBuffer.wrap(packet, 0, 8).long
                LocalizationNative.observe(id, SystemClock.elapsedRealtime(), capturedAt, packet.copyOfRange(8, packet.size))
            }
            main.post {
                if (!running) return@post
                busy = false
                result.onSuccess { report ->
                    frames++
                    observation = report
                    if (!report.isNull("valid_until") && SystemClock.elapsedRealtime() > report.getLong("valid_until"))
                        clearObservation("结果到达时画面已过期")
                    if (observation.getString("state") == "located") lastFix = JSONObject(observation.toString())
                    render(); save()
                }.onFailure { finish(it.message ?: "截图失败") }
            }
        }
    }

    private fun render() {
        overlay?.text = LocalizationReplay.describe(observation, historical = false) + "\n已观察 $frames 帧 · 点击停止"
    }
    private fun clearObservation(reason: String) {
        observation = emptyObservation(reason).put("target", observation.opt("target") ?: JSONObject.NULL)
    }
    private fun save() {
        LocalizationReports.save(this, JSONObject().put("runId", id).put("active", running).put("frames", frames)
            .put("routeSha256", routeSha).put("routeName", routeName.take(256))
            .put("observation", observation).put("lastFix", lastFix ?: JSONObject.NULL)
            .put("recordedAt", System.currentTimeMillis()).put("inputOperations", 0))
    }
    private fun finish(reason: String) {
        if (ended) return
        ended = true; running = false
        main.removeCallbacks(tick)
        clearObservation(reason)
        render(); save()
        val screen = bridge
        if (screen != null) worker.execute { runCatching { screen.cancelCapture() } }
        if (bound) { bound = false; runCatching { Shizuku.unbindUserService(args, connection, true) } }
        bridge = null
        main.postDelayed({ stopSelf() }, 15_000)
    }
    override fun onDestroy() {
        finish("定位准备已结束")
        main.removeCallbacksAndMessages(null)
        Shizuku.removeBinderDeadListener(dead)
        overlay?.let { runCatching { window.removeView(it) } }; overlay = null
        worker.shutdownNow()
        if (active.get() === this) active = WeakReference(null)
        super.onDestroy()
    }
    override fun onBind(intent: Intent?) = null
    companion object {
        private var active = WeakReference<LocalizationService>(null)
        fun stopForReturn() { active.get()?.let { service -> service.main.post { service.finish("已返回 Paimon，实时位置已撤销") } } }
        private fun emptyObservation(reason: String) = JSONObject().put("state", "unlocated").put("position", JSONObject.NULL)
            .put("heading_degrees", JSONObject.NULL).put("target", JSONObject.NULL).put("valid_until", JSONObject.NULL)
            .put("navigation_allowed", false).put("reason", reason)
    }
}
