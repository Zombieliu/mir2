//! Optional data-only Web adapter for the shared Crystal chat controller.
use mir2_client_core::chat_ui::{ChatUiState,SettingsAction,knob_top};
use wasm_bindgen::prelude::*;
#[wasm_bindgen]
pub fn chat_ui_abi_version()->u32 {1}
#[wasm_bindgen]
pub struct ChatUiBridge {state:ChatUiState}
#[wasm_bindgen]
impl ChatUiBridge {
    #[wasm_bindgen(constructor)]
    pub fn new(mask:u32)->Result<ChatUiBridge,JsValue> {
        let state=u16::try_from(mask).ok().and_then(ChatUiState::new)
            .ok_or_else(||JsValue::from_str("Invalid chat settings mask"))?;
        Ok(Self{state})
    }
    pub fn document(&self)->String {
        let s=&self.state;
        let draft=s.draft.map(|v|v.to_string()).unwrap_or_else(||"null".into());
        format!("{{\"version\":1,\"epoch\":{},\"open\":{},\"size\":{},\"lineCount\":{},\"frameIndex\":{},\"countBarIndex\":{},\"top\":{},\"height\":{},\"controlTop\":{},\"inputTop\":{},\"track\":{},\"knobTop\":{},\"index\":{},\"historyCount\":{},\"appliedMask\":{},\"draftMask\":{}}}",
            s.epoch,s.draft.is_some(),s.size.index(),s.size.line_count(),s.size.frame_index(),s.size.count_bar_index(),
            s.size.panel_top() as u32,s.size.panel_height() as u32,s.size.control_top() as u32,s.size.input_top() as u32,s.size.track() as u32,knob_top(s.size,s.index,s.count) as u32,
            s.index,s.count,s.committed,draft)
    }
    pub fn observe(&mut self,count:u32)->bool {self.state.observe(count as usize)}
    pub fn scroll(&mut self,action:u32,epoch:u32)->bool {
        u8::try_from(action).ok().is_some_and(|action|self.state.scroll(action,epoch))
    }
    pub fn drag(&mut self,y:f64,grab:f64,epoch:u32)->bool {self.state.drag(y,grab,epoch)}
    pub fn resize(&mut self,epoch:u32)->bool {self.state.resize(epoch)}
    pub fn open(&mut self,epoch:u32)->bool {self.state.open(epoch)}
    pub fn edit_filter(&mut self,channel:u32,visible:bool,epoch:u32)->bool {
        u8::try_from(channel).ok().is_some_and(|channel|self.state.edit(SettingsAction::Filter{channel,visible},epoch))
    }
    pub fn edit_all(&mut self,visible:bool,epoch:u32)->bool {self.state.edit(SettingsAction::All{visible},epoch)}
    pub fn edit_transparent(&mut self,transparent:bool,epoch:u32)->bool {self.state.edit(SettingsAction::Transparent(transparent),epoch)}
    pub fn defaults(&mut self,epoch:u32)->bool {self.state.edit(SettingsAction::Defaults,epoch)}
    pub fn apply(&mut self,epoch:u32)->i32 {self.state.apply(epoch).map(i32::from).unwrap_or(-1)}
    pub fn cancel(&mut self,epoch:u32)->bool {self.state.cancel(epoch)}
    pub fn retire(&mut self)->bool {self.state.retire()}
    pub fn restore(&mut self,mask:u32)->bool {u16::try_from(mask).ok().is_some_and(|mask|self.state.restore(mask))}
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn document_and_apply_are_complete_single_commit_data_only() {
        let mut ui=ChatUiBridge{state:ChatUiState::new(0).unwrap()};
        assert_eq!(chat_ui_abi_version(),1);
        assert!(ui.document().contains("\"lineCount\":4"));
        assert!(ui.observe(100));assert!(ui.document().contains("\"index\":96"));
        assert!(ui.resize(1));assert!(ui.document().contains("\"track\":55"));
        assert!(ui.open(2));assert!(ui.edit_filter(7,false,3));
        assert!(ui.document().contains("\"appliedMask\":0"));
        assert_eq!(ui.apply(3),128);assert_eq!(ui.apply(3),-1);
        assert!(!ui.edit_filter(8,false,4));assert!(ui.restore(0));assert!(!ui.restore(1024));
    }
}
