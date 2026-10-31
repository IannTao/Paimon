package dev.paimon.probe

import android.Manifest
import android.app.Activity
import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import android.provider.Settings
import android.text.InputType
import android.widget.*
import rikka.shizuku.Shizuku
import java.io.File

class MainActivity : Activity() {
    private val preferences by lazy { getSharedPreferences("probe-form", MODE_PRIVATE) }
    private lateinit var form: LinearLayout
    private lateinit var status: TextView
    private lateinit var x: EditText
    private lateinit var y: EditText
    private lateinit var count: EditText
    private lateinit var settings: EditText
    private lateinit var gamePackage: EditText
    private lateinit var scenario: Spinner
    private lateinit var observation: EditText
    private lateinit var accepted: Spinner
    private lateinit var obtained: Spinner
    private lateinit var continuity: Spinner
    private val permissionListener = Shizuku.OnRequestPermissionResultListener { _, _ -> runOnUiThread { refresh() } }
    private val binderListener = Shizuku.OnBinderReceivedListener { runOnUiThread { refresh() } }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        form = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(24, 24, 24, 24) }
        setContentView(ScrollView(this).apply { addView(form) })
        button("导入/查看 BetterGI 路线") { startActivity(Intent(this, RouteImportActivity::class.java)) }
        label("工单 07：有画面反馈的自动移动\n固定 2480×1116 横屏、正常绿色血条和小地图布局；运行中请勿手动操作。")
        label("07 需要悬浮窗和 Shizuku 授权；无障碍设置供旧触控实验使用。")
        status = label("")
        button("1. 悬浮窗权限") { startActivity(Intent(Settings.ACTION_MANAGE_OVERLAY_PERMISSION, Uri.parse("package:$packageName"))) }
        button("2. 无障碍设置") { startActivity(Intent(Settings.ACTION_ACCESSIBILITY_SETTINGS)) }
        button("3. 请求 Shizuku 授权") {
            runCatching {
                check(Shizuku.pingBinder()) { "请先安装 Shizuku，并通过无线调试启动" }
                Shizuku.requestPermission(1)
            }.onFailure { status.text = it.message }
        }
        button("4. 允许通知（建议）") { requestPermissions(arrayOf(Manifest.permission.POST_NOTIFICATIONS), 2) }
        val movementSettings = field("07 游戏显示设置、起点和方向记录", preferences.getString("movementSettings", "").orEmpty())
        button("启动 07 自动移动（约 0.9 秒，自动进入原神）") {
            runCatching {
                check(Settings.canDrawOverlays(this)) { "请先允许悬浮窗" }
                check(Shizuku.pingBinder() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED
                    && Shizuku.getUid() == 2000) { "请先准备 shell Shizuku 授权" }
                check(movementSettings.text.isNotBlank()) { "请先记录固定显示设置、起点和朝向" }
                val game = checkNotNull(packageManager.getLaunchIntentForPackage(MovementService.GAME)) { "未安装基准原神包" }
                preferences.edit().putString("movementSettings", movementSettings.text.toString()).apply()
                stopService(Intent(this, ProbeService::class.java))
                stopService(Intent(this, ReturnOverlayService::class.java))
                stopService(Intent(this, MovementService::class.java))
                startForegroundService(Intent(this, MovementService::class.java).putExtra("settings", movementSettings.text.toString()))
                startActivity(game)
            }.onFailure { status.text = "未启动：${it.message}" }
        }
        button("停止 07 自动移动") { stopService(Intent(this, MovementService::class.java)) }
        button("查看 07 最新结果") {
            status.text = runCatching { MovementReports.latest(this).toString(2) }
                .getOrElse { "尚无 07 任务报告" }
        }
        label("以下为旧触控共存实验，不是全自动采集入口。")
        button("显示返回 Paimon 悬浮按钮（可拖动）") {
            preferences.edit().putBoolean("returnOverlayEnabled", true).apply()
            showReturnOverlay()
        }
        button("关闭返回 Paimon 悬浮按钮") {
            preferences.edit().putBoolean("returnOverlayEnabled", false).apply()
            stopService(Intent(this, ReturnOverlayService::class.java))
        }
        label("坐标使用横屏全屏物理像素，以左上角为原点；可用系统开发者选项的指针位置查找。固定游戏显示配置后填写。")
        x = field("拾取位置 X", preferences.getString("x", "").orEmpty(), true)
        y = field("拾取位置 Y", preferences.getString("y", "").orEmpty(), true)
        count = field("本轮点击次数（2–30）", preferences.getString("count", "10").orEmpty(), true)
        gamePackage = field("原神包名", preferences.getString("gamePackage", "com.miHoYo.Yuanshen").orEmpty())
        settings = field("游戏分辨率、画质、UI 设置、横屏方向、MyOS 版本", preferences.getString("settings", "").orEmpty())
        label("本轮触控场景")
        scenario = selector(listOf("持续摇杆", "摇杆加视角双指"))
        scenario.setSelection(preferences.getInt("scenario", 0).coerceIn(0, 1))
        button("创建悬浮控制（默认暂停，不会立即点击）") { prepare() }
        button("关闭悬浮验证") { stopService(Intent(this, ProbeService::class.java)) }
        label("最新报告的人工观察（只在结束并关闭悬浮验证后填写）")
        label("游戏是否接受点击？")
        accepted = selector(listOf("unverified", "yes", "no"))
        label("是否实际取得物品？")
        obtained = selector(listOf("unverified", "yes", "no"))
        label("真实触控是否全程连续、无需重新落指？")
        continuity = selector(listOf("unverified", "yes", "no"))
        observation = field("观察：停步、摇杆复位、触点取消、重新落指、忽略点击及复现条件", "")
        button("记录观察并导出最新报告") { exportReport() }
        Shizuku.addRequestPermissionResultListener(permissionListener)
        Shizuku.addBinderReceivedListenerSticky(binderListener)
    }

    private fun prepare() {
        runCatching {
            check(Settings.canDrawOverlays(this)) { "请先允许悬浮窗" }
            check(ProbeAccessibilityService.connected) { "请先开启 Paimon 无障碍服务" }
            check(Shizuku.pingBinder() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED) { "请先完成 Shizuku 授权" }
            check(Shizuku.getUid() == 2000) { "本实验仅接受无线调试启动的 shell Shizuku" }
            check(settings.text.isNotBlank()) { "请记录游戏显示设置、方向和系统版本" }
            val px = x.text.toString().toInt(); val py = y.text.toString().toInt()
            val repetitions = count.text.toString().toInt()
            require(px >= 0 && py >= 0 && repetitions in 2..30) { "坐标或次数无效" }
            check(gamePackage.text.isNotBlank()) { "请填写游戏包名" }
            stopService(Intent(this, ProbeService::class.java))
            startForegroundService(Intent(this, ProbeService::class.java)
                .putExtra("x", px).putExtra("y", py).putExtra("count", repetitions)
                .putExtra("gamePackage", gamePackage.text.toString().trim())
                .putExtra("settings", settings.text.toString()).putExtra("scenario", scenario.selectedItem.toString()))
            status.text = "进入固定横屏的游戏画面后点击悬浮开始；5 秒倒计时内恢复摇杆/视角操作。再次点击可停止后续点击。"
        }.onFailure { status.text = "未准备：${it.message}" }
    }

    private fun exportReport() {
        runCatching {
            val report = ProbeReports.latest(this)
            check(report.has("endedAt")) { "请先结束本轮并关闭悬浮验证，再填写观察" }
            report.put("gameAccepted", accepted.selectedItem.toString())
                .put("itemObtained", obtained.selectedItem.toString())
                .put("touchContinuity", continuity.selectedItem.toString())
                .put("observerNotes", observation.text.toString())
                .put("observedAt", java.time.Instant.now().toString())
                // Overall gate needs repeated scenarios and human review; never infer it from exit codes.
                .put("overallGate", "unverified")
            val exportBody = ProbeReports.recordObservation(this, report).toString(2)
            File(filesDir, "pending-export.json").writeText(exportBody)
            startActivityForResult(Intent(Intent.ACTION_CREATE_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE)
                .setType("application/json").putExtra(Intent.EXTRA_TITLE, "paimon-${report.getString("runId")}.json"), 10)
        }.onFailure { status.text = "无法导出：${it.message}" }
    }

    @Deprecated("Android activity result callback")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == 10 && resultCode == RESULT_OK) {
            runCatching {
                val uri = requireNotNull(data?.data)
                val stream = checkNotNull(contentResolver.openOutputStream(uri))
                val exportBody = File(filesDir, "pending-export.json").readText()
                stream.bufferedWriter(Charsets.UTF_8).use { it.write(exportBody) }
            }.onSuccess { status.text = "报告已导出；整体触控门槛仍需人工复核两类重复测试。" }
                .onFailure { status.text = "导出失败：${it.message}" }
        }
    }

    override fun onResume() {
        super.onResume()
        ProbeService.pauseForReturn()
        MovementService.stopForReturn()
        LocalizationService.stopForReturn()
        if (::status.isInitialized) refresh()
        if (preferences.getBoolean("returnOverlayEnabled", true) && Settings.canDrawOverlays(this)) showReturnOverlay()
    }

    private fun showReturnOverlay() {
        if (!Settings.canDrawOverlays(this)) { status.text = "请先允许悬浮窗"; return }
        runCatching { startForegroundService(Intent(this, ReturnOverlayService::class.java)) }
            .onFailure { status.text = "无法显示返回按钮：${it.message}" }
    }

    override fun onPause() {
        preferences.edit().putString("x", x.text.toString()).putString("y", y.text.toString())
            .putString("count", count.text.toString()).putString("gamePackage", gamePackage.text.toString())
            .putString("settings", settings.text.toString()).putInt("scenario", scenario.selectedItemPosition).apply()
        super.onPause()
    }
    private fun refresh() {
        status.text = "无障碍连接：${ProbeAccessibilityService.connected}\nShizuku：" + runCatching {
            if (Shizuku.pingBinder()) "UID ${Shizuku.getUid()} / 授权 ${Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED}" else "未启动"
        }.getOrElse { "不可用" }
    }
    private fun label(value: String) = TextView(this).apply { text = value; form.addView(this) }
    private fun field(hintText: String, value: String, numeric: Boolean = false) = EditText(this).apply {
        hint = hintText; setText(value)
        inputType = if (numeric) InputType.TYPE_CLASS_NUMBER else InputType.TYPE_CLASS_TEXT
        form.addView(this)
    }
    private fun button(title: String, action: () -> Unit) { form.addView(Button(this).apply { text = title; setOnClickListener { action() } }) }
    private fun selector(items: List<String>) = Spinner(this).apply {
        adapter = ArrayAdapter(this@MainActivity, android.R.layout.simple_spinner_dropdown_item, items)
        form.addView(this)
    }
    override fun onDestroy() {
        Shizuku.removeRequestPermissionResultListener(permissionListener)
        Shizuku.removeBinderReceivedListener(binderListener)
        super.onDestroy()
    }
}
