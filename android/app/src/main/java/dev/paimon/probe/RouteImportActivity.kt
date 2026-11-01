package dev.paimon.probe

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.content.pm.PackageManager
import android.provider.Settings
import android.provider.OpenableColumns
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import android.widget.EditText
import android.widget.CheckBox
import android.text.InputType
import org.json.JSONObject
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger
import rikka.shizuku.Shizuku

/** Route preparation: file import, diagnostic replay and explicitly started screen observation. */
class RouteImportActivity : Activity() {
    private val worker = Executors.newSingleThreadExecutor()
    private val generation = AtomicInteger()
    private lateinit var content: LinearLayout
    private lateinit var status: TextView
    private lateinit var preview: LinearLayout
    private var current: JSONObject? = null
    private var sourceUri = ""
    private var page = 0
    private var capabilityPage = 0
    private var diagnosticPage = 0
    private var selected: RouteImports.Saved? = null
    private lateinit var localization: TextView
    private lateinit var segmentFirst: EditText
    private lateinit var segmentLast: EditText
    private lateinit var diagnostics: CheckBox

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        content = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(24, 24, 24, 24) }
        setContentView(ScrollView(this).apply { addView(content) })
        label(content, "BetterGI 路线导入与预检\n范围为璃月港地表、固定布局、walk及基础空动作传送。单点拾取目前只识别化种匣的琉璃百合中文提示；未知锚点、force_tp及其他动作不支持。整条路线按钮仅导航；单点拾取不代表完整采集。运行中自动操作。")
        button(content, "选择路线原文件") {
            @Suppress("DEPRECATION")
            startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                type = "*/*"
            }, PICK_ROUTE)
        }
        button(content, "读取已保存路线（离线）") { restore() }
        button(content, "启动整条受支持路线") { startNavigation(0, 0) }
        button(content, "仅验证首个传送点（非整条通过）") { startNavigation(0, 0, firstTeleportOnly = true) }
        button(content, "验证首个目标点拾取（非整条采集）") { startNavigation(0, 0, firstPickup = true) }
        segmentFirst = EditText(this).apply { hint = "分段首点序号（原数组，1起算）"; inputType = InputType.TYPE_CLASS_NUMBER; setText("2"); content.addView(this) }
        segmentLast = EditText(this).apply { hint = "分段末点序号"; inputType = InputType.TYPE_CLASS_NUMBER; setText("4"); content.addView(this) }
        diagnostics = CheckBox(this).apply { text = "保存本次诊断画面（最多160帧小地图＋64帧界面概览）"; content.addView(this) }
        button(content, "仅验证导航分段（非原路线通过）") {
            val first = segmentFirst.text.toString().toIntOrNull()
            val last = segmentLast.text.toString().toIntOrNull()
            if (first == null || last == null || first < 1 || last < first) localization.text = "分段序号无效"
            else startNavigation(first, last)
        }
        button(content, "查看上次导航结果") {
            localization.text = runCatching { NavigationReports.latest(this).toString(2) }.getOrElse { "暂无记录：${it.message}" }
        }
        button(content, "用截图检查定位（历史画面）") {
            if (selected == null || current?.optString("status") == "invalid") {
                localization.text = "请先导入合法路线"; return@button
            }
            @Suppress("DEPRECATION")
            startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).addCategory(Intent.CATEGORY_OPENABLE).setType("image/png"), PICK_IMAGE)
        }
        button(content, "开始实时定位准备（进入原神）") { startLocalization() }
        button(content, "查看上次定位记录（非当前位置）") {
            localization.text = runCatching { "历史记录，非当前位置，禁止导航\n${LocalizationReports.latest(this).toString(2)}" }
                .getOrElse { "暂无定位记录：${it.message}" }
        }
        localization = label(content, "定位尚未开始。实时准备需要首页的 shell Shizuku 和悬浮窗授权；只观察屏幕，最长45秒，不移动角色。当前仅校验 2480×1116 固定布局、璃月港地表局部参照。")
        status = label(content, "请选择本地 JSON 路线（不超过 2 MiB）。只保存最近一次合法导入。")
        preview = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        content.addView(preview)
    }

    @Deprecated("Platform Activity result API")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
        if (requestCode == PICK_IMAGE && resultCode == RESULT_OK) {
            val uri = data?.data ?: return
            val source = selected ?: return
            val request = generation.incrementAndGet()
            localization.text = "正在检查历史画面；非实时，不允许导航…"
            worker.execute {
                val result = runCatching {
                    val bytes = checkNotNull(contentResolver.openInputStream(uri)).use { it.readNBytes(8 * 1024 * 1024 + 1) }
                    LocalizationReplay.inspect(source.bytes, bytes)
                }
                runOnUiThread {
                    if (!isDestroyed && generation.get() == request) {
                        localization.text = result.fold({ LocalizationReplay.describe(it, historical = true) }, { "历史画面检查失败，未定位：${it.message}" })
                    }
                }
            }
            return
        }
        if (requestCode != PICK_ROUTE || resultCode != RESULT_OK) return
        val uri = data?.data ?: return
        prepare(save = true) {
            val name = contentResolver.query(uri, arrayOf(OpenableColumns.DISPLAY_NAME), null, null, null)?.use { cursor ->
                if (cursor.moveToFirst()) cursor.getString(cursor.getColumnIndexOrThrow(OpenableColumns.DISPLAY_NAME)) else null
            } ?: "route.json"
            val bytes = checkNotNull(contentResolver.openInputStream(uri)) { "无法读取路线文件" }.use {
                it.readNBytes(RouteNative.MAX_BYTES + 1)
            }
            require(bytes.size <= RouteNative.MAX_BYTES) { "文件超过 2 MiB" }
            RouteImports.Saved(name, uri.toString(), bytes)
        }
    }

    private fun restore() = prepare(save = false) { RouteImports(this).load() }

    private fun prepare(save: Boolean, read: () -> RouteImports.Saved) {
        val request = generation.incrementAndGet()
        current = null
        selected = null
        localization.text = "定位尚未开始"
        preview.removeAllViews()
        status.text = "正在本地读取并预检…"
        worker.execute {
            val result = runCatching {
                val source = read()
                val report = RouteNative.prepare(source.name, source.bytes)
                if (generation.get() == request && save && report.getString("status") != "invalid") {
                    RouteImports(this).save(source, report)
                }
                source to report
            }
            runOnUiThread {
                if (!isDestroyed && generation.get() == request) {
                    result.onSuccess { (source, report) ->
                        sourceUri = source.uri
                        current = report
                        selected = if (report.getString("status") != "invalid") source else null
                        page = 0
                        capabilityPage = 0
                        diagnosticPage = 0
                        render()
                    }.onFailure { status.text = "导入失败：${it.message}\n当前没有可执行路线。可重新选择，或读取上次保存的合法路线。" }
                }
            }
        }
    }

    private fun startLocalization() {
        runCatching {
            val report = checkNotNull(current) { "请先导入路线" }
            check(selected != null && report.getString("status") != "invalid") { "路线格式无效" }
            check(report.getString("map_name") == "Teyvat") { "当前只支持 Teyvat 的璃月港地表局部定位" }
            val capabilities = report.getJSONArray("capabilities")
            check((0 until capabilities.length()).any {
                val capability = capabilities.getJSONObject(it)
                capability.getString("field") == "world_localization" && capability.getBoolean("executable")
            }) { "路线不在已验证的地表区域，或包含未解释的定位配置" }
            check(Settings.canDrawOverlays(this)) { "请在首页允许悬浮窗权限" }
            check(Shizuku.pingBinder() && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED && Shizuku.getUid() == 2000) { "请在首页准备 shell Shizuku 授权" }
            val game = checkNotNull(packageManager.getLaunchIntentForPackage(MovementService.GAME)) { "未安装基准原神包" }
            stopService(Intent(this, MovementService::class.java))
            stopService(Intent(this, ProbeService::class.java))
            stopService(Intent(this, ReturnOverlayService::class.java))
            stopService(Intent(this, LocalizationService::class.java))
            startForegroundService(Intent(this, LocalizationService::class.java).putExtra("routeSha", report.getString("sha256")))
            startActivity(game)
        }.onFailure { localization.text = "定位准备未启动：${it.message}" }
    }

    private fun startNavigation(first: Int, last: Int, firstTeleportOnly: Boolean = false, firstPickup: Boolean = false) {
        val source = selected ?: run { localization.text = "请先导入路线"; return }
        val report = current ?: return
        val request = generation.incrementAndGet()
        localization.text = "正在检查导航范围…"
        worker.execute {
            val routeExecution = first == 0 && last == 0
            val result = runCatching { if (routeExecution) RouteExecutionNative.preview(source.bytes, firstTeleportOnly, firstPickup) else NavigationNative.preview(source.bytes, first, last) }
            runOnUiThread {
                if (isDestroyed || request != generation.get()) return@runOnUiThread
                result.onSuccess {
                    runCatching {
                        check(Settings.canDrawOverlays(this)) { "请先允许悬浮窗权限" }
                        check(Shizuku.pingBinder() && Shizuku.getUid() == 2000 && Shizuku.checkSelfPermission() == PackageManager.PERMISSION_GRANTED) { "需要 shell Shizuku 授权" }
                        val game = checkNotNull(packageManager.getLaunchIntentForPackage(MovementService.GAME))
                        stopService(Intent(this, MovementService::class.java))
                        stopService(Intent(this, ProbeService::class.java))
                        stopService(Intent(this, ReturnOverlayService::class.java))
                        stopService(Intent(this, LocalizationService::class.java))
                        startForegroundService(Intent(this, NavigationService::class.java)
                            .putExtra("routeSha", report.getString("sha256")).putExtra("first", first).putExtra("last", last)
                            .putExtra("routeExecution", routeExecution).putExtra("firstTeleportOnly", firstTeleportOnly)
                            .putExtra("firstPickup", firstPickup)
                            .putExtra("diagnostics", diagnostics.isChecked))
                        startActivity(game)
                    }.onFailure { localization.text = "导航未启动：${it.message}" }
                }.onFailure { localization.text = "导航未启动：${it.message}" }
            }
        }
    }

    override fun onResume() { super.onResume(); LocalizationService.stopForReturn(); NavigationService.stopForReturn() }

    private fun render() {
        val report = current ?: return
        preview.removeAllViews()
        val state = when (report.getString("status")) {
            "invalid" -> "格式错误"
            "unsupported" -> "格式合法，执行能力未齐"
            "executable" -> "格式合法，能力预检通过"
            else -> "未知预检结果，不能执行"
        }
        status.text = state
        label(preview, "文件：${report.getString("file_name")}\n来源：$sourceUri\nSHA-256：${report.getString("sha256")}\n字节：${report.getLong("byte_count")}\n原文件提交：${report.optString("upstream_commit").takeUnless { it == "null" || it.isEmpty() } ?: "未核验（本地文件）"}")
        val info = report.optJSONObject("original")?.optJSONObject("info")
        label(preview, "名称：${report.getString("name")}\n地图：${report.getString("map_name")}\n原始元数据：\n${excerpt(info?.toString(2) ?: "无")}")
        val points = report.getJSONArray("points")
        label(preview, "路径点：${points.length()}（保持原数组顺序；坐标为游戏坐标）")
        val start = page * PAGE_SIZE
        val end = minOf(start + PAGE_SIZE, points.length())
        if (points.length() > PAGE_SIZE) {
            label(preview, "显示第 ${start + 1}–$end 点 / ${points.length()} 点；已预检全部点")
            button(preview, "上一页") { page--; render() }.isEnabled = page > 0
            button(preview, "下一页") { page++; render() }.isEnabled = end < points.length()
        }
        for (i in start until end) {
            val p = points.getJSONObject(i)
            val raw = p.getJSONObject("original")
            label(preview, "第 ${i + 1} 点 · id=${if (raw.has("id")) raw.get("id") else "未提供"}\n(${p.getDouble("x")}, ${p.getDouble("y")}) ${p.getString("point_type")} / ${p.getString("move_mode")} / ${p.getString("action")}\n原始点：${excerpt(raw.toString())}")
        }
        label(preview, "所需能力（已识别 / 可执行）")
        val capabilities = report.getJSONArray("capabilities")
        for (i in range("能力", capabilities.length(), capabilityPage) { capabilityPage = it; render() }) {
            val c = capabilities.getJSONObject(i)
            val scope = when (c.getString("field")) {
                "world_localization" -> "（仅璃月港地表局部/2480×1116，须检查实时画面）"
                "type:teleport" -> "（仅已验证的璃月港锚点、空动作，启动时还须确认可用）"
                else -> ""
            }
            label(preview, "${c.getString("field")}$scope：${if (c.getBoolean("recognized")) "是" else "否"} / ${if (c.getBoolean("executable")) "是" else "否"}")
        }
        label(preview, "诊断（包含全部路径点及路线配置，独立分页）")
        val diagnostics = report.getJSONArray("diagnostics")
        for (i in range("诊断", diagnostics.length(), diagnosticPage) { diagnosticPage = it; render() }) {
            val d = diagnostics.getJSONObject(i)
            label(preview, "${d.getString("field")}：${d.getString("reason")}")
        }
        label(preview, "导航启动按钮位于页面上方；必需传送或动作未支持时禁止整条启动。分段验证单独记录，不算原路线通过。")
    }

    private fun range(name: String, count: Int, page: Int, change: (Int) -> Unit): IntRange {
        val start = page * PAGE_SIZE
        val end = minOf(start + PAGE_SIZE, count)
        label(preview, "$name：共 $count 项，显示 ${if (count == 0) 0 else start + 1}–$end 项")
        if (count > PAGE_SIZE) {
            button(preview, "${name}上一页") { change(page - 1) }.isEnabled = page > 0
            button(preview, "${name}下一页") { change(page + 1) }.isEnabled = end < count
        }
        return start until end
    }

    private fun excerpt(value: String) = if (value.length <= 4000) value else value.take(4000) + "\n（显示已截断；原文件完整保留）"
    private fun label(parent: LinearLayout, text: String) = TextView(this).apply {
        this.text = excerpt(text); textSize = 16f; setPadding(0, 12, 0, 12); setTextIsSelectable(true); parent.addView(this)
    }
    private fun button(parent: LinearLayout, text: String, action: () -> Unit) = Button(this).apply {
        this.text = text; setOnClickListener { action() }; parent.addView(this)
    }
    override fun onDestroy() { generation.incrementAndGet(); worker.shutdownNow(); super.onDestroy() }
    companion object { private const val PICK_ROUTE = 1601; private const val PICK_IMAGE = 1702; private const val PAGE_SIZE = 50 }
}
