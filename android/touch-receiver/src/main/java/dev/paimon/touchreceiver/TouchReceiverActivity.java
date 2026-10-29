package dev.paimon.touchreceiver;

import android.app.Activity;
import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.Paint;
import android.graphics.PointF;
import android.os.Bundle;
import android.os.SystemClock;
import android.view.KeyEvent;
import android.view.MotionEvent;
import android.view.View;
import android.view.WindowInsets;
import android.view.WindowManager;
import java.io.BufferedWriter;
import java.io.File;
import java.io.FileOutputStream;
import java.io.OutputStreamWriter;
import java.nio.charset.StandardCharsets;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.Map;
import org.json.JSONArray;
import org.json.JSONException;
import org.json.JSONObject;

/** Records only MotionEvents delivered to this window. It never injects input. */
public final class TouchReceiverActivity extends Activity {
    private ReceiverView receiver;

    @Override public void onCreate(Bundle state) {
        super.onCreate(state);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON);
        getWindow().setDecorFitsSystemWindows(false);
        WindowManager.LayoutParams attributes = getWindow().getAttributes();
        attributes.layoutInDisplayCutoutMode = WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES;
        getWindow().setAttributes(attributes);
        receiver = new ReceiverView();
        setContentView(receiver);
        getWindow().getInsetsController().hide(WindowInsets.Type.systemBars());
    }

    @Override public boolean onKeyDown(int code, KeyEvent event) {
        if (code == KeyEvent.KEYCODE_VOLUME_DOWN) {
            receiver.requestStop(); return true;
        }
        if (code == KeyEvent.KEYCODE_VOLUME_UP) {
            if (!receiver.recording) receiver.start();
            return true;
        }
        return super.onKeyDown(code, event);
    }

    @Override protected void onPause() {
        receiver.save("activity_paused");
        super.onPause();
    }

    @Override public boolean onKeyUp(int code, KeyEvent event) {
        if (code == KeyEvent.KEYCODE_VOLUME_UP || code == KeyEvent.KEYCODE_VOLUME_DOWN) return true;
        return super.onKeyUp(code, event);
    }

    private final class ReceiverView extends View {
        private static final int MAX_RECORDS = 12000;
        private final ArrayList<String> records = new ArrayList<>();
        private final Map<Integer, PointF> contacts = new HashMap<>();
        private final Paint paint = new Paint(Paint.ANTI_ALIAS_FLAG);
        private boolean recording;
        private boolean stopRequested;
        private boolean truncated;
        private int cancels;
        private int events;
        private int maximumPointers;
        private int lastDevice = -1;
        private long maximumDispatchDelayMs;
        private String filename;
        private String status = "";

        ReceiverView() { super(TouchReceiverActivity.this); start(); }

        private void start() {
            if (!contacts.isEmpty()) {
                status = "请先抬起全部手指，再按音量加开始新一轮";
                invalidate();
                return;
            }
            records.clear(); contacts.clear();
            stopRequested = false; truncated = false; cancels = 0; events = 0;
            maximumPointers = 0; maximumDispatchDelayMs = 0; lastDevice = -1;
            filename = "touch-" + System.currentTimeMillis() + ".jsonl";
            recording = true;
            status = "记录中：先单指拖动，再双指拖动；音量减保存，音量加新一轮";
            append(object("event", "session", "session", filename, "createdAtMs", System.currentTimeMillis()));
            invalidate();
        }

        private void append(JSONObject value) {
            if (records.size() < MAX_RECORDS) records.add(value.toString());
            else truncated = true;
        }

        @Override protected void onSizeChanged(int width, int height, int oldWidth, int oldHeight) {
            super.onSizeChanged(width, height, oldWidth, oldHeight);
            if (recording) append(object("event", "surface", "width", width, "height", height,
                    "rotation", getDisplay().getRotation()));
        }

        private JSONArray pointers(MotionEvent event, int history) {
            JSONArray points = new JSONArray();
            for (int i = 0; i < event.getPointerCount(); i++) {
                points.put(object("id", event.getPointerId(i), "toolType", event.getToolType(i),
                        "x", history < 0 ? event.getX(i) : event.getHistoricalX(i, history),
                        "y", history < 0 ? event.getY(i) : event.getHistoricalY(i, history),
                        "pressure", history < 0 ? event.getPressure(i) : event.getHistoricalPressure(i, history),
                        "size", history < 0 ? event.getSize(i) : event.getHistoricalSize(i, history)));
            }
            return points;
        }

        @Override public boolean onTouchEvent(MotionEvent event) {
            int action = event.getActionMasked();
            if (recording) {
                long received = SystemClock.uptimeMillis();
                JSONArray history = new JSONArray();
                for (int h = 0; h < event.getHistorySize(); h++) {
                    history.put(object("eventTimeMs", event.getHistoricalEventTime(h), "pointers", pointers(event, h)));
                }
                append(object("event", "motion", "action", action, "actionIndex", event.getActionIndex(),
                        "downTimeMs", event.getDownTime(), "eventTimeMs", event.getEventTime(),
                        "receivedUptimeMs", received, "deviceId", event.getDeviceId(), "source", event.getSource(),
                        "flags", event.getFlags(), "pointers", pointers(event, -1), "history", history));
                events++;
                maximumPointers = Math.max(maximumPointers, event.getPointerCount());
                maximumDispatchDelayMs = Math.max(maximumDispatchDelayMs, received - event.getEventTime());
                if (action == MotionEvent.ACTION_CANCEL) cancels++;
            }
            lastDevice = event.getDeviceId();
            contacts.clear();
            if (action != MotionEvent.ACTION_CANCEL && action != MotionEvent.ACTION_UP) {
                for (int i = 0; i < event.getPointerCount(); i++) {
                    if (action == MotionEvent.ACTION_POINTER_UP && i == event.getActionIndex()) continue;
                    contacts.put(event.getPointerId(i), new PointF(event.getX(i), event.getY(i)));
                }
            }
            if (action == MotionEvent.ACTION_UP) performClick();
            if (recording && stopRequested && contacts.isEmpty()) save("volume_down");
            invalidate();
            return true;
        }

        @Override public boolean performClick() { super.performClick(); return true; }

        private void requestStop() {
            if (!recording) return;
            stopRequested = true;
            if (contacts.isEmpty()) save("volume_down");
            else status = "请抬起全部手指，随后自动保存";
            invalidate();
        }

        private void save(String reason) {
            if (!recording) return;
            recording = false;
            JSONObject summary = object("event", "summary", "reason", reason, "motionEvents", events,
                    "maxPointers", maximumPointers, "cancelEvents", cancels, "truncated", truncated,
                    "activePointersAtStop", contacts.size(), "maxAppDispatchDelayMs", maximumDispatchDelayMs,
                    "width", getWidth(), "height", getHeight(), "rotation", getDisplay().getRotation(),
                    "coexistence", "unverified");
            try (BufferedWriter writer = new BufferedWriter(new OutputStreamWriter(
                    new FileOutputStream(new File(getFilesDir(), filename)), StandardCharsets.UTF_8))) {
                for (String line : records) { writer.write(line); writer.newLine(); }
                writer.write(summary.toString()); writer.newLine();
                status = "已保存；可继续试触，音量加新一轮（" + filename + "）";
            } catch (java.io.IOException error) { status = "保存失败：" + error.getMessage(); }
            invalidate();
        }

        @Override protected void onDraw(Canvas canvas) {
            super.onDraw(canvas);
            canvas.drawColor(Color.rgb(14, 21, 34));
            paint.setColor(Color.WHITE); paint.setTextSize(30);
            canvas.drawText("Paimon 独立触摸接收器（记录本窗口事件）", 50, 65, paint);
            paint.setTextSize(25);
            canvas.drawText(status, 50, 110, paint);
            canvas.drawText("事件 " + events + "   最多 " + maximumPointers + " 指   CANCEL " + cancels + "   设备 " + lastDevice, 50, 150, paint);
            if (truncated) canvas.drawText("缓冲区已满，本轮证据不完整", 50, 190, paint);
            for (Map.Entry<Integer, PointF> contact : contacts.entrySet()) {
                paint.setColor(Color.HSVToColor(new float[]{(contact.getKey() * 110) % 360, 0.65f, 1f}));
                PointF point = contact.getValue();
                canvas.drawCircle(point.x, point.y, 35, paint);
                paint.setTextSize(30);
                canvas.drawText("id " + contact.getKey(), point.x + 45, point.y, paint);
            }
        }
    }

    private static JSONObject object(Object... values) {
        JSONObject result = new JSONObject();
        try { for (int i = 0; i < values.length; i += 2) result.put((String) values[i], values[i + 1]); }
        catch (JSONException error) { throw new IllegalArgumentException(error); }
        return result;
    }
}
