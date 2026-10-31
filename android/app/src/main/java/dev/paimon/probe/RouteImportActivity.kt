package dev.paimon.probe

import android.app.Activity
import android.content.Intent
import android.os.Bundle
import android.provider.OpenableColumns
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import org.json.JSONObject
import java.util.concurrent.Executors
import java.util.concurrent.atomic.AtomicInteger

/** Local task preparation only: no input services, game launch or permission requests. */
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

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        content = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL; setPadding(24, 24, 24, 24) }
        setContentView(ScrollView(this).apply { addView(content) })
        label(content, "BetterGI 路线导入与预检\n预览游戏坐标，不操作游戏。定位、导航、传送和动作尚未实现，当前路线均不能执行。")
        button(content, "选择路线原文件") {
            @Suppress("DEPRECATION")
            startActivityForResult(Intent(Intent.ACTION_OPEN_DOCUMENT).apply {
                addCategory(Intent.CATEGORY_OPENABLE)
                type = "*/*"
            }, PICK_ROUTE)
        }
        button(content, "读取已保存路线（离线）") { restore() }
        status = label(content, "请选择本地 JSON 路线（不超过 2 MiB）。只保存最近一次合法导入。")
        preview = LinearLayout(this).apply { orientation = LinearLayout.VERTICAL }
        content.addView(preview)
    }

    @Deprecated("Platform Activity result API")
    override fun onActivityResult(requestCode: Int, resultCode: Int, data: Intent?) {
        super.onActivityResult(requestCode, resultCode, data)
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
                        page = 0
                        capabilityPage = 0
                        diagnosticPage = 0
                        render()
                    }.onFailure { status.text = "导入失败：${it.message}\n当前没有可执行路线。可重新选择，或读取上次保存的合法路线。" }
                }
            }
        }
    }

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
            label(preview, "${c.getString("field")}：${if (c.getBoolean("recognized")) "是" else "否"} / ${if (c.getBoolean("executable")) "是" else "否"}")
        }
        label(preview, "诊断（包含全部路径点及路线配置，独立分页）")
        val diagnostics = report.getJSONArray("diagnostics")
        for (i in range("诊断", diagnostics.length(), diagnosticPage) { diagnosticPage = it; render() }) {
            val d = diagnostics.getJSONObject(i)
            label(preview, "${d.getString("field")}：${d.getString("reason")}")
        }
        button(preview, "执行路线（尚未开放）") {}.isEnabled = false
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
    companion object { private const val PICK_ROUTE = 1601; private const val PAGE_SIZE = 50 }
}
