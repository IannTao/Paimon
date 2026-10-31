package dev.paimon.probe;

import android.content.ContentProvider;
import android.content.ContentValues;
import android.database.Cursor;
import android.database.MatrixCursor;
import android.net.Uri;
import android.os.ParcelFileDescriptor;
import android.provider.OpenableColumns;
import java.io.IOException;
import java.io.InputStream;
import java.nio.charset.StandardCharsets;

/** Debug-only local adapter; original sample assets live in the separate test APK. */
public class RouteFixtureProvider extends ContentProvider {
    @Override public boolean onCreate() { return true; }
    @Override public String getType(Uri uri) { return uri.getLastPathSegment().startsWith("image-") ? "image/png" : "application/json"; }
    @Override public Cursor query(Uri uri, String[] projection, String selection, String[] args, String sort) {
        MatrixCursor cursor = new MatrixCursor(new String[]{OpenableColumns.DISPLAY_NAME});
        cursor.addRow(new Object[]{"路线🌸-" + uri.getLastPathSegment() + ".json"});
        return cursor;
    }
    @Override public ParcelFileDescriptor openFile(Uri uri, String mode) throws java.io.FileNotFoundException {
        if (!"r".equals(mode)) throw new java.io.FileNotFoundException("read-only");
        try {
            byte[] bytes;
            String name = uri.getLastPathSegment();
            if (name.startsWith("image-")) {
                String file;
                if ("image-128".equals(name)) file = "harbor-128.png";
                else if ("image-127".equals(name)) file = "harbor-127.png";
                else if ("image-chasm".equals(name)) file = "chasm.png";
                else throw new java.io.FileNotFoundException("Unknown image fixture");
                try (InputStream input = getContext().createPackageContext("dev.paimon.probe.test", 0)
                    .getAssets().open("localization/" + file)) { bytes = input.readAllBytes(); }
            } else if ("navigation".equals(name)) {
                try (InputStream input = getContext().createPackageContext("dev.paimon.probe.test", 0)
                    .getAssets().open("navigation/derived-ground-orientation.json")) { bytes = input.readAllBytes(); }
            } else if ("ground".equals(name) || "flight".equals(name)) {
                String file = "ground".equals(name) ? "route-1.json" : "route-2.json";
                try (InputStream input = getContext().createPackageContext("dev.paimon.probe.test", 0)
                    .getAssets().open("bettergi/" + file)) { bytes = input.readAllBytes(); }
            } else if ("jsonc".equals(name)) {
                bytes = ("{/* comment */\"info\":{\"name\":\"自定义\",\"author\":\"作者\",\"version\":\"2\"},"
                    + "\"config\":{\"new_behavior\":1},\"positions\":[{\"id\":9,\"x\":1.25,\"y\":-2,},"
                    + "{\"id\":1,\"x\":1.25,\"y\":-2,\"action\":\"future\",\"action_params\":{\"n\":3}},],}").getBytes(StandardCharsets.UTF_8);
            } else if ("invalid".equals(name)) {
                bytes = "{\"info\":{},\"positions\":[{\"y\":1}]}".getBytes(StandardCharsets.UTF_8);
            } else if ("large".equals(name)) {
                StringBuilder json = new StringBuilder("{\"info\":{\"name\":\"");
                for (int i = 0; i < 50000; i++) json.append('a');
                json.append("\"},\"positions\":[{\"x\":1,\"y\":2}],\"config\":{");
                for (int i = 0; i < 2000; i++) {
                    if (i > 0) json.append(',');
                    json.append('"').append(String.format(java.util.Locale.ROOT, "key%04d", i)).append("\":1");
                }
                bytes = json.append("}}").toString().getBytes(StandardCharsets.UTF_8);
            } else if ("many-errors".equals(name)) {
                StringBuilder json = new StringBuilder("{\"info\":{},\"positions\":[");
                for (int i = 0; i < 1000; i++) {
                    if (i > 0) json.append(',');
                    json.append("{\"x\":null,\"y\":null}");
                }
                bytes = json.append("]}").toString().getBytes(StandardCharsets.UTF_8);
            } else throw new java.io.FileNotFoundException("Unknown fixture");
            ParcelFileDescriptor[] pipe = ParcelFileDescriptor.createPipe();
            final byte[] data = bytes;
            new Thread(() -> {
                try (ParcelFileDescriptor.AutoCloseOutputStream output = new ParcelFileDescriptor.AutoCloseOutputStream(pipe[1])) {
                    output.write(data);
                } catch (IOException error) { throw new RuntimeException(error); }
            }).start();
            return pipe[0];
        } catch (IOException | android.content.pm.PackageManager.NameNotFoundException error) {
            throw new java.io.FileNotFoundException(error.toString());
        }
    }
    @Override public Uri insert(Uri uri, ContentValues values) { throw new UnsupportedOperationException("read-only"); }
    @Override public int delete(Uri uri, String selection, String[] args) { throw new UnsupportedOperationException("read-only"); }
    @Override public int update(Uri uri, ContentValues values, String selection, String[] args) { throw new UnsupportedOperationException("read-only"); }
}
