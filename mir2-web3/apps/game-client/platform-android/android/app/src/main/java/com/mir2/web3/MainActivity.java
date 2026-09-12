package com.mir2.web3;

import android.os.Bundle;
import android.os.Handler;
import android.os.Looper;
import android.text.Editable;
import android.text.InputType;
import android.text.TextWatcher;
import android.view.View;
import android.view.KeyEvent;
import android.view.WindowInsets;
import android.view.WindowInsetsController;
import android.view.WindowManager;
import android.view.inputmethod.InputMethodManager;
import android.view.inputmethod.EditorInfo;
import android.widget.EditText;
import android.widget.FrameLayout;
import com.google.androidgamesdk.GameActivity;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import org.json.JSONArray;
import org.json.JSONObject;

/** Platform I/O only. Visible player UI is the shared Crystal Bevy shell. */
public final class MainActivity extends GameActivity {
    private static native void nativeEvent(String json);
    private static native String nativePoll();
    private static native void nativeGatewayHostStart();
    private static native void nativeGatewayHostStop();
    private static native void nativeGatewayConnectionLost();
    private static native String nativeGatewayPoll();
    private static native boolean nativeGatewayReport(long sequence, boolean sent);
    static { System.loadLibrary("mir2_platform_android"); }
    private final Handler handler = new Handler(Looper.getMainLooper());
    private final ForegroundRecoveryPolicy recoveryPolicy = new ForegroundRecoveryPolicy();
    private final GatewayHostPolicy gatewayHostPolicy = new GatewayHostPolicy();
    private GatewaySession session;
    private OkHttpClient client;
    private EditText ime;
    private String editing = "";
    private boolean updating, foreground, sensitiveEditor, imeWasVisible, multilineEditor;
    private volatile boolean networkReportedAvailable;

    @Override protected void onCreate(Bundle state) {
        super.onCreate(state);
        // Transparent OS input connection, not a second player-facing form.
        ime = new EditText(this);
        ime.setSingleLine(true);
        ime.setImeOptions(EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_FLAG_NO_FULLSCREEN | EditorInfo.IME_ACTION_DONE);
        ime.setOnEditorActionListener((view, action, event) -> {
            // A newline in a long-form shared draft is text, not SubmitMail or
            // a guild operation. Leave it to the normal EditText connection.
            if (multilineEditor && (action == EditorInfo.IME_NULL || action == EditorInfo.IME_ACTION_NONE)) return false;
            nativeEvent(GatewaySession.object("type", "submit", "field", editing).toString());
            hideKeyboard();
            return true;
        });
        ime.setOnApplyWindowInsetsListener((view, insets) -> {
            android.graphics.Insets safe = insets.getInsets(WindowInsets.Type.systemBars() | WindowInsets.Type.displayCutout());
            nativeEvent(GatewaySession.object("type", "insets",
                    "bottom", insets.getInsets(WindowInsets.Type.ime()).bottom,
                    "safeRight", safe.right, "safeTop", safe.top,
                    "safeLeft", safe.left, "safeBottom", safe.bottom).toString());
            boolean visible = insets.isVisible(WindowInsets.Type.ime());
            boolean dismissed = imeWasVisible && !visible;
            imeWasVisible = visible;
            // Android can consume Back to hide the IME without calling the
            // Activity. Do not consume the next Back a second time as an editor.
            if (dismissed && !editing.isEmpty()) hideKeyboard();
            return insets;
        });
        ime.setAlpha(0f);
        ime.setSaveEnabled(false);
        ime.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS);
        ime.setImportantForAccessibility(View.IMPORTANT_FOR_ACCESSIBILITY_NO);
        addContentView(ime, new FrameLayout.LayoutParams(1, 1));
        ime.addTextChangedListener(new TextWatcher() {
            public void beforeTextChanged(CharSequence s, int start, int count, int after) {}
            public void onTextChanged(CharSequence s, int start, int before, int count) {
                if (!updating && !editing.isEmpty()) {
                    if (sensitiveEditor) getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
                    nativeEvent(GatewaySession.object("type", "edit", "field", editing, "text", s.toString()).toString());
                }
            }
            public void afterTextChanged(Editable s) {}
        });
        client = new OkHttpClient.Builder().connectTimeout(12, TimeUnit.SECONDS)
                .readTimeout(25, TimeUnit.SECONDS).writeTimeout(12, TimeUnit.SECONDS)
                .pingInterval(10, TimeUnit.SECONDS).followRedirects(false).followSslRedirects(false).build();
        session = new GatewaySession(client, view -> {
            JSONArray roster = new JSONArray();
            for (GatewaySession.Character character : view.characters) {
                roster.put(GatewaySession.object("index", character.index, "name", character.name,
                        "level", character.level, "className", character.className, "genderName", character.genderName));
            }
            nativeEvent(GatewaySession.object("phase", view.phase.name(), "message", view.message,
                    "characters", roster, "world", view.world == null ? JSONObject.NULL : view.world.toJson(),
                    "worldSnapshot", view.worldSnapshot == null ? JSONObject.NULL : view.worldSnapshot).toString());
            boolean networkAvailable = view.phase != GatewaySession.Phase.DISCONNECTED
                    && view.phase != GatewaySession.Phase.CONNECTING;
            if (networkAvailable != networkReportedAvailable) {
                networkReportedAvailable = networkAvailable;
                nativeEvent(GatewaySession.object("type", "lifecycle", "state",
                        networkAvailable ? "networkAvailable" : "networkUnavailable").toString());
            }
            GatewayHostPolicy.Action hostAction = gatewayHostPolicy.observe(
                    view.phase, view.worldSnapshot != null);
            if (hostAction == GatewayHostPolicy.Action.START) {
                nativeGatewayHostStart();
            } else if (hostAction == GatewayHostPolicy.Action.INVALIDATE) {
                nativeGatewayConnectionLost();
            }
        }, receipt -> nativeEvent(GatewaySession.object(
                "type", "gatewayReceipt", "envelope", receipt).toString()),
                packet -> nativeEvent(GatewaySession.object(
                        "type", "gatewayGameplayPacket", "envelope", packet).toString()));
        connect();
        hideSystemUi();
    }

    private void connect() {
        if (BuildConfig.UI_PREVIEW) {
            nativeEvent(GatewaySession.object("type", "uiPreview", "scene",
                    getIntent().getStringExtra("ui_scene") == null ? "hud" : getIntent().getStringExtra("ui_scene")).toString());
            return;
        }
        // Explicit build-time configuration; exported intents cannot override the URL.
        if (BuildConfig.MIR2_GATEWAY_URL.isEmpty()) {
            nativeEvent(GatewaySession.object("phase", "UNCONFIGURED", "message",
                    "Test server not configured.").toString());
            return;
        }
        try { session.connect(BuildConfig.MIR2_GATEWAY_URL); }
        catch (IllegalArgumentException error) {
            nativeEvent(GatewaySession.object("phase", "UNCONFIGURED", "message", "Invalid approved WSS endpoint configuration").toString());
        }
    }

    private final Runnable pump = new Runnable() {
        @Override public void run() {
            if (!foreground) return;
            for (int i = 0; i < 16; i++) {
                String raw = nativePoll();
                if (raw.isEmpty()) break;
                try {
                    JSONObject command = new JSONObject(raw);
                    switch (command.getString("type")) {
                        case "connect": connect(); break;
                        case "login": if (!BuildConfig.UI_PREVIEW) session.login(command.getString("account"), command.getString("password")); break;
                        case "start": if (!BuildConfig.UI_PREVIEW) session.start(command.getInt("index")); break;
                        case "disconnect": session.disconnect("Disconnected. Reconnect to refresh server state."); break;
                        case "keyboard":
                            editing = command.getString("field");
                            sensitiveEditor = command.optBoolean("password") || editing.contains("account");
                            multilineEditor = command.optBoolean("multiline") && !sensitiveEditor && !command.optBoolean("numeric");
                            updating = true;
                            ime.setInputType(InputType.TYPE_CLASS_TEXT | (command.optBoolean("password", editing.equals("password"))
                                    ? InputType.TYPE_TEXT_VARIATION_PASSWORD : InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD)
                                    | InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS
                                    | (multilineEditor ? InputType.TYPE_TEXT_FLAG_MULTI_LINE : 0));
                            if (command.optBoolean("numeric")) ime.setInputType(InputType.TYPE_CLASS_NUMBER);
                            ime.setSingleLine(!multilineEditor);
                            ime.setImeOptions(EditorInfo.IME_FLAG_NO_EXTRACT_UI | EditorInfo.IME_FLAG_NO_FULLSCREEN
                                    | (multilineEditor ? EditorInfo.IME_ACTION_NONE : EditorInfo.IME_ACTION_DONE));
                            ime.setText(command.getString("text"));
                            ime.setSelection(ime.length());
                            updating = false;
                            ime.requestFocus();
                            ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).showSoftInput(ime, InputMethodManager.SHOW_IMPLICIT);
                            break;
                        case "hideKeyboard": hideKeyboard(); break;
                        case "privacy":
                            if (command.getBoolean("secure")) getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
                            else getWindow().clearFlags(WindowManager.LayoutParams.FLAG_SECURE);
                            break;
                        default: break;
                    }
                } catch (Exception error) {
                    session.disconnect("Host command failed; reconnect");
                }
            }
            if (!BuildConfig.UI_PREVIEW && gatewayHostPolicy.active()) {
                for (int i = 0; i < 16; i++) {
                    String raw = nativeGatewayPoll();
                    if (raw.isEmpty()) break;
                    boolean sent = false;
                    long sequence = 0;
                    try {
                        JSONObject envelope = new JSONObject(raw);
                        sequence = envelope.getLong("sequence");
                        sent = session.sendAuthenticated(envelope.getJSONObject("command"));
                    } catch (Exception ignored) {
                        sent = false;
                    }
                    boolean accepted = nativeGatewayReport(sequence, sent);
                    if (!accepted || !sent) {
                        session.disconnect("Gameplay send failed; reconnect and log in again");
                        break;
                    }
                }
            }
            handler.postDelayed(this, 33);
        }
    };

    private void hideKeyboard() {
        editing = "";
        updating = true;
        ime.setText("");
        updating = false;
        ((InputMethodManager)getSystemService(INPUT_METHOD_SERVICE)).hideSoftInputFromWindow(ime.getWindowToken(), 0);
        ime.clearFocus();
    }

    // GameActivity consumes Back in its native key handler (BrowserBack), so
    // Activity's default onBackPressed route is never reached. IME pre-dispatch
    // still gets first refusal; only a tracked, uncancelled key-up cancels UI.
    @Override public boolean onKeyDown(int keyCode, KeyEvent event) {
        if (keyCode == KeyEvent.KEYCODE_BACK) {
            if (event.getRepeatCount() == 0) event.startTracking();
            return true;
        }
        return super.onKeyDown(keyCode, event);
    }

    @Override public boolean onKeyUp(int keyCode, KeyEvent event) {
        if (keyCode == KeyEvent.KEYCODE_BACK) {
            if (event.isTracking() && !event.isCanceled()) onBackPressed();
            return true;
        }
        return super.onKeyUp(keyCode, event);
    }

    @Override public void onBackPressed() {
        if (!editing.isEmpty()) { hideKeyboard(); return; }
        nativeEvent(GatewaySession.object("type", "back").toString());
    }

    @Override protected void onStart() {
        super.onStart();
        foreground = true;
        nativeEvent(GatewaySession.object("type", "lifecycle", "state", "resume").toString());
        handler.post(pump);
        if (recoveryPolicy.takeReconnectOnStart()) connect();
    }
    @Override protected void onStop() {
        foreground = false;
        handler.removeCallbacks(pump);
        nativeEvent(GatewaySession.object("type", "lifecycle", "state", "pause").toString());
        hideKeyboard();
        if (recoveryPolicy.markStoppedAndShouldDisconnect(BuildConfig.UI_PREVIEW)) {
            // Never retain or replay credentials. Foreground recovery only
            // re-establishes the approved WSS transport; the player logs in
            // again through the normal shared form.
            session.disconnect("Backgrounded. Reconnecting securely; log in again to refresh server state.");
        }
        super.onStop();
    }
    @Override protected void onDestroy() {
        nativeEvent(GatewaySession.object("type", "lifecycle", "state", "destroy").toString());
        nativeGatewayHostStop();
        gatewayHostPolicy.reset();
        session.close();
        client.dispatcher().executorService().shutdown();
        client.connectionPool().evictAll();
        super.onDestroy();
    }
    @Override public void onWindowFocusChanged(boolean focused) {
        super.onWindowFocusChanged(focused);
        if (focused) hideSystemUi();
    }
    private void hideSystemUi() {
        getWindow().setDecorFitsSystemWindows(false);
        WindowInsetsController controller = getWindow().getInsetsController();
        if (controller != null) {
            controller.setSystemBarsBehavior(WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE);
            controller.hide(WindowInsets.Type.systemBars());
        }
    }
}
