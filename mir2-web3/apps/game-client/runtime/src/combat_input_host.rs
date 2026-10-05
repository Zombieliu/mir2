//! Independent ordinary combat input host; installed on every renderer backend.
use mir2_client_bevy::combat_input::{CombatController, CombatEdge, CombatOutput, CombatSnapshot};
#[cfg(target_arch = "wasm32")]
mod web {
    use super::*;
    use bevy::prelude::*;
    use js_sys::Function;
    use std::cell::{Cell, RefCell};
    use wasm_bindgen::prelude::*;
    thread_local! {
     static STATE:RefCell<CombatController>=RefCell::new(CombatController::default());
     static SINK:RefCell<Option<Function>>=const{RefCell::new(None)};
     static STARTED:Cell<bool>=const{Cell::new(false)};
     static FRAME:Cell<u64>=const{Cell::new(0)};
     static ANCHOR:Cell<(u64,f64)>=const{Cell::new((0,0.))};
     static TIME:Cell<f64>=const{Cell::new(0.)};
    }
    fn now() -> u64 {
        let (base, at) = ANCHOR.with(Cell::get);
        let delta = (TIME.with(Cell::get) - at).max(0.);
        base.saturating_add(delta.min(u64::MAX as f64) as u64)
    }
    fn publish(outputs: Vec<CombatOutput>) {
        // Release every Rust mutable borrow before invoking JS. JS may capture a new snapshot.
        let sink = SINK.with(|s| s.borrow().clone());
        let Some(sink) = sink else {
            return;
        };
        for output in outputs {
            let Ok(json) = serde_json::to_string(&output) else {
                continue;
            };
            let _ = sink.call1(&JsValue::NULL, &JsValue::from_str(&json));
        }
    }
    fn advance(time: Res<Time>) {
        TIME.with(|t| t.set(time.elapsed_secs_f64() * 1000.));
        let alive = FRAME.with(|f| {
            match f
                .get()
                .checked_add(1)
                .filter(|v| *v <= mir2_client_bevy::skill_page_state::MAX_SAFE_ID)
            {
                Some(v) => {
                    f.set(v);
                    true
                }
                None => false,
            }
        });
        if !alive {
            STATE.with(|s| s.borrow_mut().clear());
            return;
        }
        let outputs = STATE.with(|s| s.borrow_mut().tick(now(), 0));
        publish(outputs);
    }
    pub(crate) fn install(app: &mut App) {
        STARTED.with(|s| s.set(true));
        app.add_systems(Update, advance.after(crate::RuntimePresentationSet));
    }
    #[wasm_bindgen(js_name=getMir2CombatInputCapabilities)]
    pub fn capabilities() -> String {
        serde_json::json!({"schemaVersion":1,"combatInputAbiVersion":1,"combatIntentAbiVersion":1,"compiled":true,"startup":STARTED.with(Cell::get)}).to_string()
    }
    #[wasm_bindgen(js_name=setMir2CombatInputSnapshot)]
    pub fn snapshot(json: &str) -> bool {
        if !STARTED.with(Cell::get) {
            return false;
        }
        let parsed = serde_json::from_str::<CombatSnapshot>(json);
        let Ok(snapshot) = parsed else {
            // A former owner's malformed call must not revoke the current owner.
            if let Ok(value) = serde_json::from_str::<serde_json::Value>(json) {
                if let Some(identity) = value
                    .get("identity")
                    .and_then(|v| serde_json::from_value(v.clone()).ok())
                {
                    STATE.with(|s| {
                        let mut state = s.borrow_mut();
                        if state
                            .snapshot
                            .as_ref()
                            .is_some_and(|s| s.identity == identity)
                        {
                            state.clear();
                        }
                    });
                }
            }
            return false;
        };
        let at = snapshot.now_ms;
        let accepted = STATE.with(|s| s.borrow_mut().ingest(snapshot));
        if accepted {
            ANCHOR.with(|a| a.set((at, TIME.with(Cell::get))));
        }
        accepted
    }
    #[wasm_bindgen(js_name=setMir2CombatInputEdge)]
    pub fn edge(json: &str) -> String {
        let Ok(edge) = serde_json::from_str::<CombatEdge>(json) else {
            return "{\"handled\":false}".into();
        };
        let (handled, outputs) = STATE.with(|s| s.borrow_mut().edge(edge, now()));
        publish(outputs);
        serde_json::json!({"handled":handled}).to_string()
    }
    #[wasm_bindgen(js_name=getMir2CombatInputStatus)]
    pub fn status() -> String {
        STATE.with(|s|{let state=s.borrow();let snapshot=state.snapshot.as_ref();serde_json::json!({"version":1,"frame":FRAME.with(Cell::get),"ready":STARTED.with(Cell::get)&&FRAME.with(Cell::get)>0&&snapshot.is_some_and(|s|s.enabled),"identity":snapshot.map(|s|s.identity),"modelRevision":snapshot.map(|s|s.model_revision),"revision":snapshot.map(|s|s.revision),"map":snapshot.map(|s|s.map.as_str())}).to_string()})
    }
    #[wasm_bindgen(js_name=setMir2CombatInputIntentSink)]
    pub fn set_sink(sink: Function) {
        SINK.with(|s| *s.borrow_mut() = Some(sink));
    }
    #[wasm_bindgen(js_name=clearMir2CombatInputIntentSink)]
    pub fn clear_sink() {
        SINK.with(|s| *s.borrow_mut() = None);
        STATE.with(|s| s.borrow_mut().clear());
    }
    #[wasm_bindgen(js_name=withdrawMir2CombatInputSnapshot)]
    pub fn withdraw(json: &str) {
        if let Ok(identity) =
            serde_json::from_str::<mir2_client_bevy::combat_input::CombatIdentity>(json)
        {
            STATE.with(|s| {
                let mut state = s.borrow_mut();
                if state
                    .snapshot
                    .as_ref()
                    .is_some_and(|s| s.identity == identity)
                {
                    state.clear();
                }
            });
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub(crate) use web::install;
#[cfg(not(target_arch = "wasm32"))]
pub(crate) fn install(_app: &mut bevy::prelude::App) {}
#[cfg(test)]
#[path = "combat_input_host_tests.rs"]
mod tests;
