//! Shared Crystal chat presentation rules. No renderer, I/O or gateway authority.
pub const SETTINGS_MASK: u16 = 1023;
pub const DIALOG_FILTER_MASK: u16 = 255;
pub const TRANSPARENT_MASK: u16 = 512;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChatWindowSize { #[default] Small, Medium, Large }
impl ChatWindowSize {
    pub fn index(self) -> u32 { match self { Self::Small=>0, Self::Medium=>1, Self::Large=>2 } }
    pub fn line_count(self) -> usize { match self { Self::Small=>4, Self::Medium=>7, Self::Large=>11 } }
    pub fn frame_index(self) -> u16 { 2221 + self.index() as u16 * 3 }
    pub fn count_bar_index(self) -> u16 { 2012 + self.index() as u16 }
    pub fn vertical_offset(self) -> f32 { self.index() as f32 * 48.0 }
    pub fn next(self) -> Self { match self { Self::Small=>Self::Medium, Self::Medium=>Self::Large, Self::Large=>Self::Small } }
    pub fn track(self) -> f32 { 7.0 + self.vertical_offset() }
    pub fn panel_top(self)->f32 {671.0-self.vertical_offset()}
    pub fn panel_height(self)->f32 {68.0+self.vertical_offset()}
    pub fn control_top(self)->f32 {656.0-self.vertical_offset()}
    pub fn input_top(self)->f32 {54.0+self.vertical_offset()}
}
pub fn max_scroll_offset(count: usize) -> usize { count.saturating_sub(1) }
pub fn clamp_scroll_offset(index: usize, count: usize) -> usize { index.min(max_scroll_offset(count)) }
pub fn index_at_track(y: f32, grab_y: f32, track: f32, count: usize) -> usize {
    if track <= 0.0 || count <= 1 { return 0; }
    (((y-grab_y-16.0).clamp(0.0,track)/track)*(count-1) as f32).trunc() as usize
}
pub fn observe_history(mut index:usize,count:usize,line_count:usize,previous:Option<usize>,was_at_bottom:bool)->(usize,bool) {
    let max=max_scroll_offset(count);
    let follow=count.saturating_sub(line_count);
    if let Some(previous)=previous {
        if was_at_bottom && count>previous {index=follow;}
        if index>max {index=max;}
    } else if count>line_count {index=follow;}
    (index,index==follow)
}
pub fn scroll_index(index:usize,count:usize,action:u8)->Option<usize> {
    Some(match action {0=>0,1=>index.saturating_sub(1),
        2=>if index<max_scroll_offset(count){index+1}else{index},
        3=>max_scroll_offset(count),_=>return None})
}
pub fn knob_top(size: ChatWindowSize, index: usize, count: usize) -> f32 {
    if count > 1 { 16.0 + (size.track() / (count-1) as f32 * clamp_scroll_offset(index,count) as f32).trunc() }
    else { 16.0 }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SettingsAction {
    Filter { channel:u8, visible:bool }, All { visible:bool },
    Transparent(bool), Apply, Cancel, Defaults,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SettingsTransition { pub committed:u16, pub draft:Option<u16>, pub applied:Option<u16> }
/// A closed settings dialog is inert. Defaults and edits affect only its draft.
pub fn settings_transition(committed:u16, draft:Option<u16>, action:SettingsAction) -> SettingsTransition {
    let mut next=SettingsTransition { committed, draft, applied:None };
    let Some(mut value)=draft else { return next; };
    match action {
        SettingsAction::Filter {channel,visible} if channel<9 => {
            let bit=1u16<<channel; if visible {value &= !bit;} else {value |= bit;}
        }
        SettingsAction::All {visible} => { if visible {value &= !DIALOG_FILTER_MASK;} else {value |= DIALOG_FILTER_MASK;} }
        SettingsAction::Transparent(transparent) => { if transparent {value |= TRANSPARENT_MASK;} else {value &= !TRANSPARENT_MASK;} }
        SettingsAction::Defaults => value=0,
        SettingsAction::Apply => {next.committed=value; next.draft=None; next.applied=Some(value);return next;}
        SettingsAction::Cancel => {next.draft=None;return next;}
        _ => {}
    }
    next.draft=Some(value);next
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChatUiState {
    pub epoch:u32, pub size:ChatWindowSize, pub index:usize, pub count:usize,
    pub committed:u16, pub draft:Option<u16>, previous_count:Option<usize>, was_at_bottom:bool,
}
impl ChatUiState {
    pub fn new(mask:u16) -> Option<Self> {
        (mask<=SETTINGS_MASK).then_some(Self {epoch:1,size:ChatWindowSize::Small,index:0,count:0,committed:mask,draft:None,previous_count:None,was_at_bottom:false})
    }
    fn current(&self,epoch:u32)->bool { self.epoch==epoch && self.epoch<u32::MAX }
    fn advance(&mut self) {self.epoch+=1;}
    pub fn observe(&mut self,count:usize)->bool {
        if self.epoch==u32::MAX {return false;}
        let (index,bottom)=observe_history(self.index,count,self.size.line_count(),self.previous_count,self.was_at_bottom);
        self.index=index;self.was_at_bottom=bottom;self.previous_count=Some(count);self.count=count;true
    }
    pub fn scroll(&mut self,action:u8,epoch:u32)->bool {
        if !self.current(epoch)||self.draft.is_some() {return false;}
        let Some(index)=scroll_index(self.index,self.count,action) else {return false;};
        self.index=index;self.was_at_bottom=index==self.count.saturating_sub(self.size.line_count());true
    }
    pub fn drag(&mut self,y:f64,grab:f64,epoch:u32)->bool {
        if !self.current(epoch)||self.draft.is_some()||!y.is_finite()||!grab.is_finite()
            || y.abs()>f32::MAX as f64 || grab.abs()>f32::MAX as f64 {return false;}
        self.index=index_at_track(y as f32,grab as f32,self.size.track(),self.count);
        self.was_at_bottom=self.index==self.count.saturating_sub(self.size.line_count());true
    }
    pub fn resize(&mut self,epoch:u32)->bool {
        if !self.current(epoch)||self.draft.is_some() {return false;}
        self.size=self.size.next();self.index=clamp_scroll_offset(self.index,self.count);
        self.was_at_bottom=self.index==self.count.saturating_sub(self.size.line_count());self.advance();true
    }
    pub fn open(&mut self,epoch:u32)->bool {
        if !self.current(epoch)||self.draft.is_some() {return false;}
        self.draft=Some(self.committed);self.advance();true
    }
    pub fn edit(&mut self,action:SettingsAction,epoch:u32)->bool {
        if !self.current(epoch)||self.draft.is_none() {return false;}
        if matches!(action,SettingsAction::Filter{channel,..} if channel>=8)
            ||matches!(action,SettingsAction::Apply|SettingsAction::Cancel) {return false;}
        let next=settings_transition(self.committed,self.draft,action);
        self.draft=next.draft;true
    }
    pub fn apply(&mut self,epoch:u32)->Option<u16> {
        if !self.current(epoch)||self.draft.is_none() {return None;}
        let next=settings_transition(self.committed,self.draft,SettingsAction::Apply);
        self.committed=next.committed;self.draft=next.draft;self.advance();next.applied
    }
    pub fn cancel(&mut self,epoch:u32)->bool {
        if !self.current(epoch)||self.draft.is_none() {return false;}
        self.draft=None;self.advance();true
    }
    pub fn retire(&mut self)->bool {
        if self.epoch==u32::MAX {return false;}
        self.draft=None;self.index=0;self.count=0;self.previous_count=None;self.was_at_bottom=false;self.advance();true
    }
    pub fn restore(&mut self,mask:u16)->bool {
        if self.epoch==u32::MAX||self.draft.is_some()||mask>SETTINGS_MASK {return false;}
        self.committed=mask;self.advance();true
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test] fn original_track_reaches_last_and_retains_grab_offset() {
        assert_eq!(index_at_track(16.0,0.0,30.0,100),0);
        assert_eq!(index_at_track(31.0,0.0,30.0,100),49);
        assert_eq!(index_at_track(49.0,3.0,30.0,100),99);
        assert_eq!(index_at_track(99.0,0.0,0.0,100),0);
        assert_eq!(index_at_track(99.0,0.0,30.0,0),0);
    }
    #[test] fn three_sizes_preserve_frames_track_and_last_index() {
        let mut ui=ChatUiState::new(0).unwrap();ui.observe(100);assert_eq!(ui.index,96);ui.scroll(3,ui.epoch);assert_eq!(ui.index,99);
        for (size,lines,frame,bar,track) in [(0,4,2221,2012,7.0),(1,7,2224,2013,55.0),(2,11,2227,2014,103.0)] {
            assert_eq!((ui.size.index(),ui.size.line_count(),ui.size.frame_index(),ui.size.count_bar_index(),ui.size.track()),(size,lines,frame,bar,track));
            let epoch=ui.epoch;assert!(ui.resize(epoch));assert!(!ui.resize(epoch));
        }
        assert_eq!(ui.size,ChatWindowSize::Small);ui.observe(3);assert_eq!(ui.index,2);
        assert_eq!(observe_history(0,100,4,None,false),(96,true));
        assert_eq!(observe_history(96,101,4,Some(100),true),(97,true));
        assert_eq!(observe_history(99,101,4,Some(100),false),(99,false));
        assert_eq!(observe_history(99,3,4,Some(100),false),(2,false));
        assert_eq!(observe_history(0,4,4,Some(4),true),(0,true));
    }
    #[test] fn apply_once_cancel_and_defaults_have_exact_draft_effects() {
        let mut ui=ChatUiState::new(256).unwrap();assert!(ui.open(1));assert!(!ui.open(1));
        let epoch=ui.epoch;assert!(ui.edit(SettingsAction::All{visible:false},epoch));
        assert!(ui.edit(SettingsAction::Transparent(true),epoch));assert_eq!(ui.committed,256);
        assert_eq!(ui.apply(epoch),Some(1023));assert_eq!(ui.apply(epoch),None);
        assert!(ui.open(ui.epoch));let epoch=ui.epoch;
        assert!(ui.edit(SettingsAction::Defaults,epoch));assert_eq!(ui.draft,Some(0));assert_eq!(ui.committed,1023);
        assert!(ui.cancel(epoch));assert_eq!(ui.committed,1023);assert!(!ui.cancel(epoch));
    }
    #[test] fn old_epoch_retired_dialog_and_nonfinite_drag_are_inert() {
        let mut ui=ChatUiState::new(0).unwrap();ui.observe(10);ui.scroll(3,ui.epoch);let old=ui.epoch;ui.open(old);
        assert!(!ui.drag(20.0,0.0,old));assert!(!ui.edit(SettingsAction::Defaults,old));
        let epoch=ui.epoch;ui.retire();assert!(!ui.edit(SettingsAction::Defaults,epoch));
        assert!(!ui.drag(f64::NAN,0.0,ui.epoch));assert!(!ui.drag(20.0,f64::INFINITY,ui.epoch));
        assert_eq!(ui.index,0);assert!(ChatUiState::new(1024).is_none());
    }
    #[test] fn settings_all_preserves_trade_and_closed_actions_have_no_effect() {
        let next=settings_transition(256,Some(256),SettingsAction::All{visible:false});
        assert_eq!(next.draft,Some(511));assert_eq!(next.committed,256);assert_eq!(next.applied,None);
        assert_eq!(settings_transition(1,None,SettingsAction::Apply),SettingsTransition{committed:1,draft:None,applied:None});
    }
}
