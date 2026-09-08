package com.mir2.web3;

import android.os.Bundle;
import android.graphics.Color;
import android.text.InputType;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.view.WindowManager;
import android.widget.ArrayAdapter;
import android.widget.Button;
import android.widget.EditText;
import android.widget.LinearLayout;
import android.widget.Spinner;
import android.widget.TextView;
import java.util.concurrent.TimeUnit;
import okhttp3.OkHttpClient;
import android.view.WindowInsets;
import android.view.WindowInsetsController;

import com.google.androidgamesdk.GameActivity;

public final class MainActivity extends GameActivity {
    private GatewaySession session;
    private OkHttpClient client;
    private EditText endpoint, account, password;
    private Spinner roster;
    private Button connect, login, enter;
    private TextView status;
    private boolean destroyed;
    private GatewaySession.Phase lastPhase;
    private static native void nativeStatus(String text);
    static {
        System.loadLibrary("mir2_platform_android");
    }

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
        client = new OkHttpClient.Builder()
                .connectTimeout(12, TimeUnit.SECONDS)
                .readTimeout(25, TimeUnit.SECONDS)
                .writeTimeout(12, TimeUnit.SECONDS)
                .pingInterval(10, TimeUnit.SECONDS)
                .followRedirects(false).followSslRedirects(false).build();
        buildLoginPanel();
        nativeStatus("Connect to your approved test Gateway");
        session = new GatewaySession(client, view -> runOnUiThread(() -> render(view)));
        hideSystemUi();
    }

    private EditText field(LinearLayout panel, String hint, boolean secret) {
        EditText input = new EditText(this);
        input.setHint(hint);
        input.setSingleLine(true);
        input.setTextColor(Color.WHITE);
        input.setHintTextColor(Color.LTGRAY);
        input.setSaveEnabled(false);
        input.setImportantForAutofill(View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS);
        input.setInputType(InputType.TYPE_CLASS_TEXT | (secret
                ? InputType.TYPE_TEXT_VARIATION_PASSWORD : InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD));
        panel.addView(input);
        return input;
    }

    private Button button(LinearLayout panel, String text, View.OnClickListener action) {
        Button result = new Button(this);
        result.setText(text);
        result.setOnClickListener(action);
        panel.addView(result);
        return result;
    }

    private void buildLoginPanel() {
        LinearLayout panel = new LinearLayout(this);
        panel.setOrientation(LinearLayout.VERTICAL);
        panel.setPadding(16, 8, 16, 8);
        panel.setBackgroundColor(0xE6102030);
        endpoint = field(panel, "Approved wss:// Gateway /ws", false);
        account = field(panel, "Test account", false);
        password = field(panel, "Password (not saved)", true);
        // Only a non-secret endpoint is remembered. Credentials are always entered by the user.
        endpoint.setText(getPreferences(MODE_PRIVATE).getString("gateway", ""));
        connect = button(panel, "Connect / retry", v -> {
            try {
                String address = endpoint.getText().toString().trim();
                GatewaySession.endpoint(address);
                getPreferences(MODE_PRIVATE).edit().putString("gateway", address).apply();
                session.connect(address);
            } catch (IllegalArgumentException error) {
                status.setText("Enter WSS endpoint without credentials, query or fragment");
            }
        });
        login = button(panel, "Log in", v -> {
            session.login(account.getText().toString().trim(), password.getText().toString());
            password.setText("");
        });
        roster = new Spinner(this);
        panel.addView(roster);
        enter = button(panel, "Enter world", v -> {
            Object selected = roster.getSelectedItem();
            if (selected instanceof GatewaySession.Character) session.start(((GatewaySession.Character) selected).index);
        });
        button(panel, "Disconnect", v -> session.disconnect("Disconnected. Log in again."));
        status = new TextView(this);
        status.setTextColor(Color.WHITE);
        status.setText("Configure approved test Gateway");
        panel.addView(status);
        login.setEnabled(false);
        enter.setEnabled(false);
        android.widget.FrameLayout.LayoutParams layout = new android.widget.FrameLayout.LayoutParams(
                (int) (350 * getResources().getDisplayMetrics().density), ViewGroup.LayoutParams.MATCH_PARENT,
                Gravity.START | Gravity.TOP);
        android.widget.ScrollView scroll = new android.widget.ScrollView(this);
        scroll.addView(panel);
        addContentView(scroll, layout);
    }

    private void render(GatewaySession.View view) {
        if (destroyed) return;
        if (lastPhase != view.phase) {
            android.util.Log.i("Mir2NativeSession", "phase=" + view.phase.name());
            lastPhase = view.phase;
        }
        status.setText(view.message);
        nativeStatus(view.message);
        login.setEnabled(view.phase == GatewaySession.Phase.READY);
        enter.setEnabled(view.phase == GatewaySession.Phase.CHARACTERS && !view.characters.isEmpty());
        ArrayAdapter<GatewaySession.Character> adapter = new ArrayAdapter<>(this,
                android.R.layout.simple_spinner_dropdown_item, view.characters);
        roster.setAdapter(adapter);
        // Allow evidence capture only after secrets have been cleared from the form.
        if (view.phase == GatewaySession.Phase.IN_GAME) {
            password.setText("");
            password.setVisibility(View.GONE);
            account.setVisibility(View.GONE);
            getWindow().clearFlags(WindowManager.LayoutParams.FLAG_SECURE);
        } else {
            getWindow().addFlags(WindowManager.LayoutParams.FLAG_SECURE);
            password.setVisibility(View.VISIBLE);
            account.setVisibility(View.VISIBLE);
        }
    }

    @Override protected void onStop() {
        if (session != null) session.disconnect("Backgrounded. Reconnect and log in to refresh server state.");
        if (password != null) password.setText("");
        super.onStop();
    }

    @Override protected void onDestroy() {
        destroyed = true;
        if (session != null) session.close();
        if (client != null) {
            client.dispatcher().executorService().shutdown();
            client.connectionPool().evictAll();
        }
        super.onDestroy();
    }

    @Override
    public void onWindowFocusChanged(boolean hasFocus) {
        super.onWindowFocusChanged(hasFocus);
        if (hasFocus) {
            hideSystemUi();
        }
    }

    private void hideSystemUi() {
        getWindow().setDecorFitsSystemWindows(false);
        WindowInsetsController controller = getWindow().getInsetsController();
        if (controller == null) {
            return;
        }
        controller.setSystemBarsBehavior(
                WindowInsetsController.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        );
        controller.hide(WindowInsets.Type.systemBars());
    }
}
