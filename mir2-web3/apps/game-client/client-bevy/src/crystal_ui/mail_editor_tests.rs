use super::*;

fn key(code: KeyCode, text: &str) -> KeyboardInput {
    KeyboardInput {
        key_code: code,
        logical_key: bevy::input::keyboard::Key::Character(text.into()),
        state: ButtonState::Pressed,
        text: (!text.is_empty()).then_some(text.into()),
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}

fn release(code: KeyCode) -> KeyboardInput {
    KeyboardInput {
        key_code: code,
        logical_key: bevy::input::keyboard::Key::Character("".into()),
        state: ButtonState::Released,
        text: None,
        repeat: false,
        window: Entity::PLACEHOLDER,
    }
}

fn draft(message: &str) -> mir2_ui_core::state::MailComposeDraft {
    mir2_ui_core::state::MailComposeDraft {
        recipient: "Receiver".into(),
        message: message.into(),
        ..default()
    }
}
#[test]
fn mail_actual_editor_commit_paste_cut_aba_and_clear_keep_persistent_checked_generation(){
    let clock=mir2_client_core::mail_compose::MailDraftClock::default();
    let mut editor=MailLetterEditor::default();editor.bind_draft_clock(clock.clone());
    let mut draft=draft("A");editor.sync(true,Some(&draft.message));let original=clock.generation();
    editor.edit_key(&key(KeyCode::ArrowLeft,""),&mut draft);editor.select_all();editor.set_composition("preedit".into(),None);
    assert_eq!(clock.generation(),original,"caret/selection/preedit alone are not content edits");
    editor.clear_composition();editor.paste(&mut draft,"B");assert_eq!(draft.message,"B");let b=clock.generation();assert_ne!(b,original);
    editor.select_all();editor.paste(&mut draft,"A");assert_eq!(draft.message,"A");assert_ne!(clock.generation(),original,"A to B to A in one callback batch invalidates the old proof");
    let before_cut=clock.generation();editor.select_all();editor.cut_selection(&mut draft);assert_eq!(draft.message,"");assert_ne!(clock.generation(),before_cut);
    let before_clear=clock.generation();editor.clear();editor.sync(true,Some("replacement"));assert_eq!(clock.generation(),before_clear,"visual clear/rebuild never resets highwater");
    editor.edit_key(&key(KeyCode::KeyZ,"Z"),&mut draft);assert!(clock.generation()>before_clear);
}

fn line(y: f32, bytes: &[usize]) -> VisualLine {
    VisualLine {
        y,
        height: 20.0,
        stops: bytes
            .iter()
            .enumerate()
            .map(|(index, byte)| CaretStop {
                byte: *byte,
                // Deliberately uneven advances: keyboard movement must use
                // captured glyph positions rather than character estimates.
                x: [0.0, 11.0, 39.0, 72.0].get(index).copied().unwrap_or(96.0),
            })
            .collect(),
    }
}

#[test]
fn unicode_selection_delete_and_ctrl_a_keep_grapheme_boundaries() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("a👩‍👩‍👧‍👦e\u{301}z");
    editor.sync(true, Some(&draft.message));

    editor.edit_key(&key(KeyCode::Backspace, ""), &mut draft);
    assert_eq!(draft.message, "a👩‍👩‍👧‍👦e\u{301}");
    editor.edit_key(&key(KeyCode::ArrowLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::Delete, ""), &mut draft);
    assert_eq!(draft.message, "a👩‍👩‍👧‍👦");

    editor.edit_key(&key(KeyCode::ControlLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::KeyA, "a"), &mut draft);
    editor.edit_key(&release(KeyCode::ControlLeft), &mut draft);
    editor.edit_key(&key(KeyCode::KeyN, "新"), &mut draft);
    assert_eq!(draft.message, "新");
    assert_eq!(editor.active_editor().unwrap().selection(), "新".len().."新".len());
}

#[test]
fn home_end_arrows_and_shift_use_captured_visual_layout_and_scroll_to_caret() {
    let message = (0..12).map(|index| format!("{index}\n")).collect::<String>();
    let mut editor = MailLetterEditor::default();
    let mut draft = draft(&message);
    editor.sync(true, Some(&draft.message));
    let mut starts = message.match_indices('\n').map(|(index, _)| index + 1).collect::<Vec<_>>();
    starts.insert(0, 0);
    let mut lines = Vec::new();
    for (index, start) in starts.iter().copied().enumerate() {
        let end = message[start..]
            .find('\n')
            .map(|offset| start + offset)
            .unwrap_or(message.len());
        lines.push(line(index as f32 * 20.0, &[start, end]));
    }
    editor.install_layout(lines);

    editor.edit_key(&key(KeyCode::ControlLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::End, ""), &mut draft);
    editor.edit_key(&release(KeyCode::ControlLeft), &mut draft);
    assert_eq!(editor.active_editor().unwrap().caret(), message.len());
    assert!(editor.scroll().y > 0.0, "bottom caret must scroll into the real viewport");

    editor.edit_key(&key(KeyCode::ArrowUp, ""), &mut draft);
    let first_up = editor.active_editor().unwrap().caret();
    editor.edit_key(&key(KeyCode::ShiftLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::ArrowUp, ""), &mut draft);
    editor.edit_key(&release(KeyCode::ShiftLeft), &mut draft);
    assert!(editor.active_editor().unwrap().selection().start < first_up);

    editor.edit_key(&key(KeyCode::ControlLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::Home, ""), &mut draft);
    editor.edit_key(&release(KeyCode::ControlLeft), &mut draft);
    assert_eq!(editor.active_editor().unwrap().caret(), 0);
}

#[test]
fn pointer_hit_test_uses_uneven_shaped_stops_and_selection_extends() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("abcd");
    editor.sync(true, Some(&draft.message));
    editor.install_layout(vec![line(0.0, &[0, 1, 3, 4])]);

    editor.pointer(Vec2::new(35.0, 4.0), false);
    assert_eq!(editor.active_editor().unwrap().caret(), 3);
    editor.pointer(Vec2::new(10.0, 4.0), true);
    assert_eq!(editor.active_editor().unwrap().selection(), 1..3);
}

#[test]
fn wrapped_boundary_keeps_the_chosen_downward_line_and_scroll_affinity() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("abc");
    editor.sync(true, Some(&draft.message));
    editor.install_layout(vec![
        VisualLine {
            y: 160.0,
            height: 20.0,
            stops: vec![
                CaretStop { byte: 0, x: 0.0 },
                CaretStop { byte: 2, x: 37.0 },
            ],
        },
        VisualLine {
            y: 180.0,
            height: 20.0,
            stops: vec![
                CaretStop { byte: 2, x: 37.0 },
                CaretStop { byte: 3, x: 58.0 },
            ],
        },
    ]);

    editor.pointer(Vec2::new(37.0, 164.0), false);
    assert_eq!(editor.visual_line, 0);
    editor.edit_key(&key(KeyCode::ArrowDown, ""), &mut draft);
    assert_eq!(editor.active_editor().unwrap().caret(), 2);
    assert_eq!(editor.visual_line, 1, "the shared wrap byte retains downward affinity");
    assert!(editor.scroll().y > 0.0, "scrolling follows the chosen second visual line");
}

#[test]
fn retained_letter_draft_keeps_selection_through_a_transient_modal() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("letter body");
    editor.sync(true, Some(&draft.message));
    editor.install_layout(vec![line(0.0, &[0, 6, draft.message.len()])]);
    editor.pointer(Vec2::new(11.0, 4.0), false);
    editor.pointer(Vec2::new(72.0, 4.0), true);
    let selection = editor.active_editor().unwrap().selection();

    // The parent overlay leaves the editor intact while a recipient/feedback
    // layer temporarily covers the same retained Letter draft.
    editor.sync(true, Some(&draft.message));
    assert_eq!(editor.active_editor().unwrap().selection(), selection);
}

#[test]
fn wheel_uses_shaped_extent_clamps_and_preserves_caret_and_selection() {
    let message = "x\n".repeat(20);
    let mut editor = MailLetterEditor::default();
    let mut draft = draft(&message);
    editor.sync(true, Some(&draft.message));
    let mut starts = message
        .match_indices('\n')
        .map(|(index, _)| index + 1)
        .collect::<Vec<_>>();
    starts.insert(0, 0);
    editor.install_layout(
        starts
            .iter()
            .copied()
            .enumerate()
            .map(|(index, start)| {
                let end = message[start..]
                    .find('\n')
                    .map(|offset| start + offset)
                    .unwrap_or(message.len());
                line(index as f32 * 20.0, &[start, end])
            })
            .collect(),
    );
    let caret = editor.active_editor().unwrap().caret();
    let selection = editor.active_editor().unwrap().selection();

    assert!(editor.scroll_wheel_lines(-100.0));
    assert_eq!(editor.scroll().y, 259.0, "wheel clamps to shaped bottom");
    assert_eq!(editor.active_editor().unwrap().caret(), caret);
    assert_eq!(editor.active_editor().unwrap().selection(), selection);
    // Text layout capture happens every render frame. Re-capturing identical
    // shaped geometry must retain a manual wheel position instead of snapping
    // to the unchanged caret.
    editor.accept_layout(
        EditorTextLayout {
            lines: starts
                .iter()
                .copied()
                .enumerate()
                .map(|(index, start)| {
                    let end = message[start..]
                        .find('\n')
                        .map(|offset| start + offset)
                        .unwrap_or(message.len());
                    line(index as f32 * 20.0, &[start, end])
                })
                .collect(),
        },
        message.clone(),
    );
    assert_eq!(editor.scroll().y, 259.0, "unchanged frame capture keeps wheel scroll");
    assert!(!editor.scroll_wheel_lines(-1.0), "bottom cannot overscroll");

    editor.accept_layout(
        EditorTextLayout {
            lines: starts
                .iter()
                .copied()
                .enumerate()
                .map(|(index, start)| {
                    let end = message[start..]
                        .find('\n')
                        .map(|offset| start + offset)
                        .unwrap_or(message.len());
                    line(index as f32 * 10.0, &[start, end])
                })
                .collect(),
        },
        message.clone(),
    );
    assert_eq!(editor.scroll().y, 59.0, "new shaped extent clamps retained wheel scroll");

    assert!(editor.scroll_wheel_pixels(10_000.0));
    assert_eq!(editor.scroll().y, 0.0, "wheel clamps to shaped top");
}


#[test]
fn host_clipboard_cut_and_paste_share_selection_and_utf16_budget() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("keep remove");
    editor.sync(true, Some(&draft.message));
    editor.edit_key(&key(KeyCode::ControlLeft, ""), &mut draft);
    editor.edit_key(&key(KeyCode::KeyA, "a"), &mut draft);
    editor.edit_key(&release(KeyCode::ControlLeft), &mut draft);
    assert_eq!(editor.selected_text(), "keep remove");
    editor.cut_selection(&mut draft);
    assert_eq!(draft.message, "");

    editor.paste(&mut draft, &"😀".repeat(251));
    assert_eq!(draft.message, "😀".repeat(250));
    assert_eq!(draft.message.encode_utf16().count(), MAIL_LETTER_BODY_LIMIT);
}

#[test]
fn ime_preedit_is_presentation_only_and_uses_real_authoritative_caret_layout() {
    let mut editor = MailLetterEditor::default();
    let mut draft = draft("abcd");
    editor.sync(true, Some(&draft.message));
    editor.install_layout(vec![line(0.0, &[0, 1, 3, 4])]);
    editor.pointer(Vec2::new(39.0, 4.0), false);
    editor.set_composition("你好".into(), Some((3, 6)));

    assert_eq!(draft.message, "abcd");
    assert_eq!(editor.active_editor().unwrap().text(), "abcd");
    assert!(editor.composition().is_some());
    assert_eq!(editor.ime_caret(), Some(Vec2::new(39.0, 20.0)));

    editor.clear_composition();
    editor.paste(&mut draft, "你好");
    assert_eq!(draft.message, "abc你好d");
}


#[test]
fn preedit_display_layout_rewraps_candidate_and_scroll_without_mutating_draft() {
    let mut editor = MailLetterEditor::default();
    let draft = draft("a");
    editor.sync(true, Some(&draft.message));
    editor.modifiers[0] = true;
    editor.set_composition("甲乙".into(), None);
    assert_eq!(editor.modifiers, [false; 4], "composition cannot retain Ctrl");
    let display = "a甲乙".to_owned();
    editor.accept_display_layout(
        EditorTextLayout {
            lines: vec![
                line(0.0, &[0, 1]),
                VisualLine {
                    y: 200.0,
                    height: 20.0,
                    stops: vec![
                        CaretStop { byte: 1, x: 0.0 },
                        CaretStop { byte: display.len(), x: 45.0 },
                    ],
                },
            ],
        },
        display,
        "a甲乙".len(),
    );

    assert_eq!(draft.message, "a");
    assert_eq!(editor.ime_caret(), Some(Vec2::new(45.0, 220.0)));
    assert_eq!(editor.scroll().y, 59.0, "preedit layout clamps using its wrapped extent");

    editor.modifiers[1] = true;
    editor.clear_composition();
    assert_eq!(editor.modifiers, [false; 4], "clear also drops stale Ctrl state");
}

mod common_compose_candidate {
    use super::*;
    use crate::crystal_ui::mail_compose_shared::*;
    #[cfg(not(feature="native-ui"))] use crate::portable_mail_ui::PortableComposeSurface;
    fn owner()->ComposeIdentity {ComposeIdentity{run:1,connection_generation:2,session_generation:3,
        owner_revision:0,scene_revision:4,hud_generation:5,player_object_id:6}}
    fn raw(body:&str)->ComposeFullRaw {ComposeFullRaw{to:"Receiver".into(),subject:" untouched subject ".into(),
        body:body.into(),gold_text:"0007".into(),items:vec!["真实UID7".into()],attachment_unique_ids:vec!["7".into()],
        stamped:true,attachment_unique_ids_present:true,stamped_present:true}}
    fn proof(e:&MailLetterEditor,sequence:u64)->TextProof {TextProof{owner:owner(),incarnation:7,draft_epoch:8,
        draft_generation:"9".into(),editor_revision:e.revision(),focus_generation:e.focus_generation(),
        presentation_revision:10,layout_revision:e.layout_revision(),sequence}}
    fn mounted(body:&str)->(MailLetterEditor,mir2_ui_core::state::MailComposeDraft) {
        let mut e=MailLetterEditor::default();let d=draft(body);
        assert!(e.mount_external(body,9,MAIL_LETTER_BODY_RECT,11));(e,d)
    }
    fn apply(a:&mut CommonMailTextAdapter,e:&mut MailLetterEditor,d:&mut mir2_ui_core::state::MailComposeDraft,
        seq:u64,op:TextOperation)->Option<TextIntent> {
        let p=proof(e,seq);let base=raw(&d.message);
        a.apply(&p,TextEdge{proof:p.clone(),operation:op},e,d,&base)
    }
    #[test]
    fn common_whole_raw_reject_and_interactive_fit_prefix_are_distinct_at_499_500_501() {
        for count in [499,500,501] {
            let (mut e,mut d)=mounted("old");let base=raw("old");let p=proof(&e,1);let mut a=CommonMailTextAdapter::default();
            let text="中".repeat(count);
            let out=a.apply(&p,TextEdge{proof:p.clone(),operation:TextOperation::ReplaceBody{text:text.clone()}},&mut e,&mut d,&base).unwrap();
            assert_eq!(d.message,text);assert_eq!(out.base_raw,base);assert_eq!(out.body_mutation,Some(text.clone()));
            assert_eq!(out.base_raw.subject," untouched subject ");assert_eq!(out.base_raw.gold_text,"0007");
            let submit=apply(&mut a,&mut e,&mut d,2,TextOperation::Submit).unwrap();
            assert_eq!(submit.action,if count>500{"rejected"}else{"submit"});assert_eq!(d.message,text);
        }
        for input in ["😀".repeat(251),format!("{}👩‍👩‍👧‍👦","a".repeat(499)),format!("{}e\u{301}","a".repeat(499))] {
            let(mut e,mut d)=mounted("");e.paste(&mut d,&input);
            assert!(d.message.encode_utf16().count()<=500);assert!(input.starts_with(&d.message));
            assert!(e.active_editor().unwrap().is_boundary(d.message.len()));
        }
        let(mut e,mut d)=mounted("");e.paste(&mut d,"a\r\nb\rc\t\0\n");assert_eq!(d.message,"a\nb\nc\n");
    }
    #[test]
    fn common_utf16_utf8_maps_reject_surrogate_halves_and_combining_selection() {
        let text="a😀e\u{301}👩‍👩‍👧‍👦\r\n中";
        for (byte,_) in text.char_indices().chain(std::iter::once((text.len(),'\0'))) {
            let units=byte_to_utf16(text,byte).unwrap();assert_eq!(utf16_to_byte(text,units),Some(byte));
        }
        assert_eq!(utf16_to_byte(text,2),None);assert_eq!(byte_to_utf16(text,2),None);
        let(mut e,_)=mounted(text);assert!(e.set_dom_selection(0,4).is_none(),"combining grapheme cannot split");
        assert!(e.set_dom_selection(1,3).is_some());assert_eq!(e.selected_text(),"😀");
    }
    #[test]
    fn common_ime_preview_cancel_commit_once_and_stale_owner_focus_are_nonmutating() {
        let(mut e,mut d)=mounted("a");let mut a=CommonMailTextAdapter::default();
        apply(&mut a,&mut e,&mut d,1,TextOperation::ImeStart{composition_id:1}).unwrap();
        apply(&mut a,&mut e,&mut d,2,TextOperation::ImePreview{composition_id:1,text:"你好".into(),cursor_utf16:Some([1,2])}).unwrap();
        assert_eq!(d.message,"a");assert_eq!(e.active_editor().unwrap().text(),"a");
        apply(&mut a,&mut e,&mut d,3,TextOperation::ImeCancel{composition_id:1}).unwrap();assert!(e.composition().is_none());
        apply(&mut a,&mut e,&mut d,4,TextOperation::ImeStart{composition_id:2}).unwrap();
        apply(&mut a,&mut e,&mut d,5,TextOperation::ImeCommit{composition_id:2,text:"你好".into()}).unwrap();
        assert_eq!(d.message,"a你好");assert!(apply(&mut a,&mut e,&mut d,6,TextOperation::ImeCommit{composition_id:2,text:"你好".into()}).is_none());
        apply(&mut a,&mut e,&mut d,7,TextOperation::ImeStart{composition_id:3}).unwrap();
        let old=proof(&e,8);e.set_focused(false);let current=proof(&e,8);let base=raw(&d.message);
        assert!(a.apply(&current,TextEdge{proof:old,operation:TextOperation::ImeCommit{composition_id:3,text:"stale".into()}},&mut e,&mut d,&base).is_none());
        assert_eq!(d.message,"a你好");
    }
    #[test]
    fn common_clipboard_copy_success_precedes_cut_and_latest_request_survives_old_response() {
        let(mut e,mut d)=mounted("keep😀");e.select_all();let mut a=CommonMailTextAdapter::default();
        let request=apply(&mut a,&mut e,&mut d,1,TextOperation::ClipboardRequest{request_id:1,kind:"cut".into()}).unwrap();
        assert_eq!(request.selected_text.as_deref(),Some("keep😀"));assert_eq!(d.message,"keep😀");
        assert!(apply(&mut a,&mut e,&mut d,2,TextOperation::ClipboardResult{request_id:1,success:false,text:None}).is_none());assert_eq!(d.message,"keep😀");
        apply(&mut a,&mut e,&mut d,3,TextOperation::ClipboardRequest{request_id:2,kind:"paste".into()}).unwrap();
        apply(&mut a,&mut e,&mut d,4,TextOperation::ClipboardRequest{request_id:3,kind:"paste".into()}).unwrap();
        assert!(apply(&mut a,&mut e,&mut d,5,TextOperation::ClipboardResult{request_id:2,success:true,text:Some("old".into())}).is_none());
        apply(&mut a,&mut e,&mut d,6,TextOperation::ClipboardResult{request_id:3,success:true,text:Some("😀".repeat(251))}).unwrap();
        assert_eq!(d.message,"😀".repeat(250));
        assert!(apply(&mut a,&mut e,&mut d,7,TextOperation::ClipboardResult{request_id:3,success:true,text:Some("duplicate".into())}).is_none());
    }
    #[test]
    fn common_async_capture_rejects_each_owner_draft_editor_focus_and_layout_field() {
        for field in 0..9 {
            let(mut e,mut d)=mounted("safe");let mut a=CommonMailTextAdapter::default();
            apply(&mut a,&mut e,&mut d,1,TextOperation::ClipboardRequest{request_id:1,kind:"paste".into()}).unwrap();
            let old=proof(&e,2);let mut current=old.clone();
            match field {0=>current.owner.connection_generation+=1,1=>current.owner.scene_revision+=1,2=>current.incarnation+=1,
                3=>current.draft_epoch+=1,4=>current.draft_generation="10".into(),5=>current.editor_revision+=1,
                6=>current.focus_generation+=1,7=>current.presentation_revision+=1,_=>current.layout_revision+=1}
            let base=raw("safe");
            assert!(a.apply(&current,TextEdge{proof:old,operation:TextOperation::ClipboardResult{request_id:1,success:true,text:Some("bad".into())}},&mut e,&mut d,&base).is_none());
            assert_eq!(d.message,"safe","capture field {field}");
        }
    }
    #[test]
    fn common_resize_font_revision_invalidates_shape_without_content_clock_and_counters_poison() {
        let clock=mir2_client_core::mail_compose::MailDraftClock::default();let mut e=MailLetterEditor::default();e.bind_draft_clock(clock.clone());
        e.sync(true,Some("abc"));e.install_layout(vec![line(0.,&[0,1,2,3])]);let before=clock.generation();
        assert!(e.configure_view(CrystalRect::new(0.,0.,100.,44.),1));assert_eq!(e.content_size(),Vec2::new(96.,40.));
        assert!(!e.layout_ready());assert_eq!(clock.generation(),before);
        assert!(!e.configure_view(CrystalRect::new(0.,0.,200.,44.),1));assert!(e.configure_view(CrystalRect::new(0.,0.,200.,44.),2));
        e.focus_generation=SAFE;e.set_focused(false);e.set_focused(true);assert!(!e.focused());assert_eq!(e.focus_generation(),0);
        e.revision=SAFE;e.bump_revision();e.bump_revision();assert_eq!(e.revision(),0);
    }
    #[cfg(not(feature="native-ui"))]
    fn computed_app(presentation:ComposePresentation,kind:ComposeKind,stamped:bool)->App {
        let body="a😀e\u{301}אבג 中文\n".repeat(15);
        computed_app_with_body(presentation,kind,stamped,&body)
    }
    #[cfg(not(feature="native-ui"))]
    fn computed_app_with_body(presentation:ComposePresentation,kind:ComposeKind,stamped:bool,body:&str)->App {
        use bevy::camera::{ComputedCameraValues,RenderTargetInfo,Viewport};
        let mut app=App::new();
        app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin::default(),bevy::input::InputPlugin,
            bevy::image::ImagePlugin::default(),bevy::transform::TransformPlugin,bevy::camera::visibility::VisibilityPlugin,
            bevy::text::TextPlugin,bevy::ui::UiPlugin));
        app.init_asset::<Image>().init_asset::<Font>().init_asset::<bevy::image::TextureAtlasLayout>()
            .init_asset::<bevy::mesh::Mesh>().init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
        let bytes=std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),"/../../web/public/original-ui/fonts/NotoSansCJKsc-Regular.otf")).expect("pinned packaged font");
        let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::from_bytes(bytes));
        let size=UVec2::new(presentation.logical_width.round()as u32,presentation.logical_height.round()as u32);
        let camera=app.world_mut().spawn((Camera2d,Camera{computed:ComputedCameraValues{target_info:Some(RenderTargetInfo{physical_size:size,scale_factor:1.}),..default()},
            viewport:Some(Viewport{physical_size:size,..default()}),..default()})).id();
        app.insert_resource(crate::crystal_ui::hud::SharedHudSurface{camera,window:Entity::PLACEHOLDER,font:font.clone(),active:true,generation:5,revision:1});
        let layout=ComposeLayout::for_presentation(presentation,kind).unwrap();
        let(mut e,mut d)=mounted(body);d.attachment_unique_ids=vec![7];
        assert!(e.configure_view(layout.body,12));e.set_dom_selection(0,3).unwrap();
        app.insert_resource(e);
        app.insert_resource(PortableComposeSurface{owner:owner(),revision:1,incarnation:7,draft_epoch:8,draft_generation:"9".into(),presentation_revision:10,layout_revision:12,
            active:true,ready:true,input_enabled:true,kind:Some(kind),layout:Some(layout),presentation:Some(presentation),raw:Some(raw(&body)),draft:d,
            parcel:ParcelPaint{stamped,stamp_available:true,quote_ready:false,postage:Some(7),cells:vec![ParcelCell{unique_id:7,image:Some(116),count_label:"2".into()}],..default()},..default()});
        app.add_plugins(crate::portable_mail_ui::Mir2PortableMailComposeUiPlugin);
        app.add_systems(Update,show_fixture.after(crate::portable_mail_ui::paint_compose));
        for _ in 0..6{app.update();}app
    }
    #[cfg(not(feature="native-ui"))]
    fn show_fixture(mut roots:Query<&mut Visibility,With<crate::portable_mail_ui::PortableComposeRoot>>) {
        for mut visibility in &mut roots{*visibility=Visibility::Inherited;}
    }
    #[test]
    #[cfg(not(feature="native-ui"))]
    fn common_actual_painter_computed_viewport_font_wrap_caret_selection_and_pointer_controls() {
        for(width,height,scale)in [(844.,390.,1.),(640.,359.298,1.),(600.,320.,1.),(1200.,640.,0.5)] {
            for stamped in [false,true] {
                let presentation=ComposePresentation{logical_width:width,logical_height:height,stage_css_scale:scale,touch:true};
                let mut app=computed_app(presentation,ComposeKind::Parcel,stamped);let w=app.world_mut();
                let (tag,node,font,computed)=w.query::<(&MailLetterEditText,&Node,&TextFont,&ComputedNode)>().single(w).unwrap();
                let expected=w.resource::<PortableComposeSurface>().layout.as_ref().unwrap().body;
                assert_eq!(tag.viewport,[expected.width-4.,expected.height-4.]);assert_eq!(node.width,Val::Px(tag.viewport[0]));
                assert_eq!(font.font,bevy::text::FontSource::Handle(w.resource::<crate::crystal_ui::hud::SharedHudSurface>().font.clone()));
                assert_eq!(font.font_size,FontSize::Px(w.resource::<PortableComposeSurface>().layout.as_ref().unwrap().body_font_px));
                assert!(w.resource::<PortableComposeSurface>().layout.as_ref().unwrap().body_font_px*scale>=13.99);
                assert!((computed.size().x-tag.viewport[0]).abs()<1.);
                assert!(computed.size().x>0.);assert!(w.resource::<MailLetterEditor>().layout_ready());
                let e=w.resource::<MailLetterEditor>();assert!(e.layout.lines.len()>1,"actual Unicode font wrapping");
                let editor_debug=format!("scroll={:?}, visual_line={}, line={:?}",e.scroll,e.visual_line,e.layout.lines.get(e.visual_line));
                assert!(w.query::<&MailEditorSelection>().iter(w).count()>0);assert_eq!(w.query::<&MailEditorCaret>().iter(w).count(),1);
                let measured=|n:&ComputedNode,t:&bevy::ui::UiGlobalTransform|{let size=n.size()*n.inverse_scale_factor;let c=t.affine().translation*n.inverse_scale_factor;CrystalRect::new(c.x-size.x*0.5,c.y-size.y*0.5,size.x,size.y)};
                let viewport=w.query_filtered::<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<MailEditorViewport>>().single(w).map(|(n,t)|measured(n,t)).unwrap();
                let caret=w.query_filtered::<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<MailEditorCaret>>().single(w).map(|(n,t)|measured(n,t)).unwrap();
                assert!(caret.left>=viewport.left+1.&&caret.top>=viewport.top+1.&&caret.left+caret.width<=viewport.left+viewport.width-1.&&caret.top+caret.height<=viewport.top+viewport.height-1.,"presentation={presentation:?}, stamped={stamped}, viewport={viewport:?}, caret={caret:?}, {editor_debug}");
                assert!(caret.left>=viewport.left+2.&&caret.top>=viewport.top+2.&&caret.left+caret.width<=viewport.left+viewport.width-2.&&caret.top+caret.height<=viewport.top+viewport.height-2.,"host content bounds: {presentation:?} {caret:?} {viewport:?}");
                let scroll=w.resource::<MailLetterEditor>().scroll_offset();
                let text_rect=w.query_filtered::<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<MailLetterEditText>>().single(w).map(|(n,t)|measured(n,t)).unwrap();
                assert!((text_rect.left-viewport.left-2.+scroll.x).abs()<0.01&&(text_rect.top-viewport.top-2.+scroll.y).abs()<0.01,"shaped text and pointer origin: {text_rect:?} {viewport:?} {scroll:?}");
                for(n,t)in w.query_filtered::<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<MailEditorSelection>>().iter(w){
                    let selection=measured(n,t);let epsilon=0.0001;assert!(selection.left+epsilon>=viewport.left+2.&&selection.top+epsilon>=viewport.top+2.&&selection.left+selection.width<=viewport.left+viewport.width-2.+epsilon&&selection.top+selection.height<=viewport.top+viewport.height-2.+epsilon,"selection content bounds: {selection:?} {viewport:?}");
                }
                let controls=w.query::<(&crate::portable_mail_ui::PortableComposeAction,&ComputedNode,&bevy::ui::UiGlobalTransform)>().iter(w)
                    .map(|(a,n,t)|{let size=n.size()*n.inverse_scale_factor;let c=t.affine().translation*n.inverse_scale_factor;
                        (a.0,CrystalRect::new(c.x-size.x*0.5,c.y-size.y*0.5,size.x,size.y))}).collect::<Vec<_>>();
                for(action,r)in &controls {if !matches!(action,ComposeAction::Body){assert!(r.width*scale>=43.9&&r.height*scale>=43.9,"{action:?} {r:?}");}}
                for(i,(_,r))in controls.iter().enumerate(){
                    assert!(r.left*scale>=11.9&&r.top*scale>=11.9&&(r.left+r.width)*scale<=width*scale-11.9&&(r.top+r.height)*scale<=height*scale-11.9);
                    assert!(controls[..i].iter().all(|(_,a)|r.left>=(a.left+a.width)||a.left>=(r.left+r.width)||r.top>=(a.top+a.height)||a.top>=(r.top+r.height)),"actual computed controls overlap");
                }
                assert_eq!(controls.iter().filter(|(a,_)|matches!(a,ComposeAction::Slot(_))).count(),if stamped{5}else{1});
                let (action,r)=controls.iter().find(|(a,_)|*a==ComposeAction::Cancel).copied().unwrap();
                let(mut editor,draft)=mounted("safe");let mut router=ComposePointerRouter::default();let base=raw("safe");
                let edge=|seq,phase:&str,x,y|ComposePointerEdge{proof:proof(&editor,seq),pointer_id:1,phase:phase.into(),x,y,button:0,shift:false};
                let center=r.center();let down=edge(1,"down",center.0,center.1);let current=down.proof.clone();
                assert!(router.handle(&current,down,&controls,&mut editor,&draft,&base).is_none());
                let up=ComposePointerEdge{proof:proof(&editor,2),pointer_id:1,phase:"up".into(),x:center.0,y:center.1,button:0,shift:false};
                assert_eq!(router.handle(&up.proof.clone(),up,&controls,&mut editor,&draft,&base).unwrap().action,"cancel");
                let down=ComposePointerEdge{proof:proof(&editor,3),pointer_id:1,phase:"down".into(),x:r.left+r.width,y:center.1,button:0,shift:false};
                assert!(router.handle(&down.proof.clone(),down,&[(action,r)],&mut editor,&draft,&base).is_none());
                let up=ComposePointerEdge{proof:proof(&editor,4),pointer_id:1,phase:"up".into(),x:center.0,y:center.1,button:0,shift:false};
                assert!(router.handle(&up.proof.clone(),up,&[(action,r)],&mut editor,&draft,&base).is_none());
            }
        }
    }
    #[test]
    fn common_held_press_rejects_resize_revision_and_unsupported_presentations() {
        let(mut e,d)=mounted("safe");let mut router=ComposePointerRouter::default();let base=raw("safe");
        let controls=vec![(ComposeAction::Cancel,CrystalRect::new(10.,10.,44.,44.))];let p=proof(&e,1);
        let down=ComposePointerEdge{proof:p.clone(),pointer_id:1,phase:"down".into(),x:32.,y:32.,button:0,shift:false};
        router.handle(&p,down,&controls,&mut e,&d,&base);assert!(e.configure_view(CrystalRect::new(0.,0.,180.,100.),12));
        let up=ComposePointerEdge{proof:proof(&e,2),pointer_id:1,phase:"up".into(),x:32.,y:32.,button:0,shift:false};
        assert!(router.handle(&up.proof.clone(),up,&controls,&mut e,&d,&base).is_none());
        for(w,h)in [(599.,320.),(600.,319.),(390.,844.)] {assert!(ComposeLayout::for_presentation(ComposePresentation{logical_width:w,logical_height:h,stage_css_scale:1.,touch:true},ComposeKind::Letter).is_none());}
    }
    #[cfg(not(feature="native-ui"))]
    #[test]
    fn common_actual_shaped_wheel_scroll_and_resize_keep_full_raw_and_clock() {
        let p=ComposePresentation{logical_width:600.,logical_height:320.,stage_css_scale:1.,touch:true};
        let mut app=computed_app(p,ComposeKind::Parcel,true);
        let raw_before=app.world().resource::<PortableComposeSurface>().raw.clone();
        let revision=app.world().resource::<MailLetterEditor>().revision();
        let clock=app.world().resource::<MailLetterEditor>().draft_clock.generation();
        {let mut e=app.world_mut().resource_mut::<MailLetterEditor>();assert!(e.scroll_wheel_pixels(-10000.));
            assert_eq!(e.scroll.y,e.max_scroll_y().unwrap());assert_eq!(e.revision(),revision);}
        for _ in 0..6{app.update();}
        let q=ComposePresentation{logical_width:844.,logical_height:390.,..p};let layout=ComposeLayout::for_presentation(q,ComposeKind::Parcel).unwrap();
        {let w=app.world_mut();let mut e=w.resource_mut::<MailLetterEditor>();assert!(e.configure_view(layout.body,13));assert!(!e.layout_ready());assert_eq!(e.scroll,Vec2::ZERO);}
        {let mut c=app.world_mut().resource_mut::<PortableComposeSurface>();c.presentation=Some(q);c.layout=Some(layout.clone());c.layout_revision=13;c.presentation_revision+=1;c.revision+=1;}
        {let w=app.world_mut();let camera=w.resource::<crate::crystal_ui::hud::SharedHudSurface>().camera;let mut camera=w.get_mut::<Camera>(camera).unwrap();
            camera.computed.target_info.as_mut().unwrap().physical_size=UVec2::new(844,390);camera.viewport.as_mut().unwrap().physical_size=UVec2::new(844,390);}
        for _ in 0..6{app.update();}
        let w=app.world_mut();let tag=w.query::<&MailLetterEditText>().single(w).unwrap();assert_eq!(tag.layout_revision,13);
        assert_eq!(tag.viewport,[layout.body.width-4.,layout.body.height-4.]);assert!(w.resource::<MailLetterEditor>().layout_ready());
        assert_eq!(w.resource::<PortableComposeSurface>().raw,raw_before);assert_eq!(w.resource::<MailLetterEditor>().draft_clock.generation(),clock);
    }
    #[cfg(not(feature="native-ui"))]
    #[test]
    fn common_actual_painter_wrapped_caret_scroll_visibility_and_content_origin() {
        for(width,height,scale)in [(600.,320.,1.),(1200.,640.,0.5)] {
            let p=ComposePresentation{logical_width:width,logical_height:height,stage_css_scale:scale,touch:true};
            let body="a😀e\u{301}אבג中文".repeat(50);
            let mut app=computed_app_with_body(p,ComposeKind::Parcel,true,&body);
            let raw_before=app.world().resource::<PortableComposeSurface>().raw.clone();
            let clock=app.world().resource::<MailLetterEditor>().draft_clock.generation();
            let selection_before=app.world().resource::<MailLetterEditor>().active_editor().unwrap().selection();
            {let mut e=app.world_mut().resource_mut::<MailLetterEditor>();
                assert!(e.layout.lines.len()>1,"actual soft wrapping without newline");
                assert!(e.scroll_wheel_pixels(-10000.));assert!(e.scroll_offset().y>e.layout.lines[0].y+e.layout.lines[0].height);}
            for _ in 0..6{app.update();}
            {let w=app.world_mut();assert_eq!(w.query::<&MailEditorCaret>().iter(w).count(),0,"offscreen caret must stay hidden after wheel");
                assert_eq!(w.resource::<MailLetterEditor>().active_editor().unwrap().selection(),selection_before);}
            {let mut e=app.world_mut().resource_mut::<MailLetterEditor>();let end=e.active_editor().unwrap().text().encode_utf16().count();e.set_dom_selection(end,end).unwrap();}
            for _ in 0..6{app.update();}
            let w=app.world_mut();
            let measured=|n:&ComputedNode,t:&bevy::ui::UiGlobalTransform|{let size=n.size()*n.inverse_scale_factor;let c=t.affine().translation*n.inverse_scale_factor;CrystalRect::new(c.x-size.x*0.5,c.y-size.y*0.5,size.x,size.y)};
            let viewport=w.query_filtered::<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<MailEditorViewport>>().single(w).map(|(n,t)|measured(n,t)).unwrap();
            let caret=w.query_filtered::<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<MailEditorCaret>>().single(w).map(|(n,t)|measured(n,t)).unwrap();
            assert!(caret.left>=viewport.left+2.&&caret.top>=viewport.top+2.&&caret.left+caret.width<=viewport.left+viewport.width-2.&&caret.top+caret.height<=viewport.top+viewport.height-2.,"scrolled caret content bounds: {p:?} {caret:?} {viewport:?}");
            let scroll=w.resource::<MailLetterEditor>().scroll_offset();
            let text=w.query_filtered::<(&ComputedNode,&bevy::ui::UiGlobalTransform),With<MailLetterEditText>>().single(w).map(|(n,t)|measured(n,t)).unwrap();
            assert!((text.left-viewport.left-2.+scroll.x).abs()<0.01&&(text.top-viewport.top-2.+scroll.y).abs()<0.01,"text origin after scroll: {text:?} {viewport:?} {scroll:?}");
            assert_eq!(w.resource::<PortableComposeSurface>().raw,raw_before);assert_eq!(w.resource::<MailLetterEditor>().draft_clock.generation(),clock);
        }
    }
    #[test]
    fn common_native_and_portable_unicode_transcript_share_editor_without_second_content_clock() {
        let clock=mir2_client_core::mail_compose::MailDraftClock::default();
        let mut native=MailLetterEditor::default();native.bind_draft_clock(clock.clone());native.sync(true,Some("a😀e\u{301}אבג\n中"));
        native.configure_view(ComposeLayout::desktop(ComposeKind::Letter).body,11);
        let(mut portable,mut pd)=mounted("a😀e\u{301}אבג\n中");let mut nd=pd.clone();
        let mut na=CommonMailTextAdapter::default();let mut pa=CommonMailTextAdapter::default();let original=clock.generation();
        for(seq,op)in [(1,TextOperation::Selection{anchor_utf16:0,caret_utf16:1}),
            (2,TextOperation::Insert{text:"中文😀".into()}),(3,TextOperation::Key{key:"end".into(),control:true,shift:false}),
            (4,TextOperation::Insert{text:"\r\n👩\u{200d}💻".into()}),(5,TextOperation::Key{key:"backspace".into(),control:false,shift:false})] {
            let before_native=nd.message.clone();let before_portable=pd.message.clone();
            let np=proof(&native,seq);let pp=proof(&portable,seq);
            let ni=na.apply(&np,TextEdge{proof:np.clone(),operation:op.clone()},&mut native,&mut nd,&raw(&before_native));
            let pi=pa.apply(&pp,TextEdge{proof:pp.clone(),operation:op},&mut portable,&mut pd,&raw(&before_portable));
            assert_eq!(nd,pd);assert_eq!(native.active_editor(),portable.active_editor());
            assert_eq!(ni.map(|i|(i.action,i.body_mutation)),pi.map(|i|(i.action,i.body_mutation)));
        }
        assert!(clock.generation()>original);assert_eq!(portable.draft_clock.generation(),Some(1),"external owner never seeds/advances Native Arc clock");
    }
    #[test]
    fn common_text_wire_rejects_unknown_duplicate_missing_nullable_and_surrogate_halves() {
        let(e,_)=mounted("😀");let edge=TextEdge{proof:proof(&e,1),operation:TextOperation::ImePreview{composition_id:1,text:"😀".into(),cursor_utf16:None}};
        let value=serde_json::to_value(edge).unwrap();let mut extra=value.clone();extra["operation"]["unexpected"]=true.into();assert!(parse_text_edge(&extra.to_string()).is_none());
        let mut missing=value.clone();missing["operation"].as_object_mut().unwrap().remove("cursorUtf16");assert!(parse_text_edge(&missing.to_string()).is_none());
        let duplicate=value.to_string().replacen("\"compositionId\":1","\"compositionId\":1,\"compositionId\":2",1);assert!(parse_text_edge(&duplicate).is_none());
        let json=value.to_string();assert!(parse_text_edge(&json).is_some());assert_eq!(utf16_to_byte("😀",1),None);
    }
    #[test]
    fn common_unrelated_ime_commit_does_not_consume_current_preview_and_prompt_capture_is_not_content() {
        let(mut e,mut d)=mounted("safe");let mut a=CommonMailTextAdapter::default();
        apply(&mut a,&mut e,&mut d,1,TextOperation::ImeStart{composition_id:2}).unwrap();
        assert!(apply(&mut a,&mut e,&mut d,2,TextOperation::ImeCommit{composition_id:1,text:"bad".into()}).is_none());
        apply(&mut a,&mut e,&mut d,3,TextOperation::ImePreview{composition_id:2,text:"中".into(),cursor_utf16:Some([1,1])}).unwrap();
        apply(&mut a,&mut e,&mut d,4,TextOperation::ImeCommit{composition_id:2,text:"中".into()}).unwrap();assert!(d.message.contains('中'));
        let raw_before=d.message.clone();let clock=e.draft_clock.generation();let revision=e.revision();let focus=e.focus_generation();
        e.retire_text_capture();e.retire_focus_capture();assert!(e.revision()>revision);assert!(e.focus_generation()>focus);
        assert_eq!(d.message,raw_before);assert_eq!(e.draft_clock.generation(),clock);
    }
    #[cfg(feature="native-ui")]
    #[derive(Resource)]struct NativeFixture {camera:Entity,font:Handle<Font>,draft:mir2_ui_core::state::MailComposeDraft}
    #[cfg(feature="native-ui")]
    #[derive(Component)]struct NativeFixtureRoot(u64);
    #[cfg(feature="native-ui")]
    #[derive(Component)]struct NativeFixtureAction(ComposeAction);
    #[cfg(feature="native-ui")]
    fn paint_native_fixture(mut commands:Commands,f:Res<NativeFixture>,e:Res<MailLetterEditor>,roots:Query<(Entity,&NativeFixtureRoot)>) {
        if roots.iter().any(|(_,r)|r.0==e.paint_revision()){return;}
        for(entity,_)in &roots{commands.entity(entity).despawn();}
        commands.spawn((NativeFixtureRoot(e.paint_revision()),bevy::ui::UiTargetCamera(f.camera),
            bevy::ui::LayoutConfig{use_rounding:false},Visibility::Inherited,
            Node{position_type:PositionType::Absolute,left:Val::Px(100.),top:Val::Px(100.),width:Val::Px(236.),height:Val::Px(300.),..default()}))
            .with_children(|p|paint_compose(p,None,ComposeKind::Letter,&f.draft,&ParcelPaint::default(),None,Some(&e),
                &ComposeLayout::desktop(ComposeKind::Letter),&TextFont{font:f.font.clone().into(),..default()},NativeFixtureAction));
    }
    #[cfg(feature="native-ui")]
    #[test]
    fn common_native_actual_app_uses_same_painter_font_shape_clip_and_source_geometry() {
        use bevy::camera::{ComputedCameraValues,RenderTargetInfo,Viewport};
        let mut app=App::new();app.add_plugins((MinimalPlugins,bevy::asset::AssetPlugin::default(),bevy::input::InputPlugin,
            bevy::image::ImagePlugin::default(),bevy::transform::TransformPlugin,bevy::camera::visibility::VisibilityPlugin,bevy::text::TextPlugin,bevy::ui::UiPlugin));
        app.init_asset::<Image>().init_asset::<Font>().init_asset::<bevy::image::TextureAtlasLayout>()
            .init_asset::<bevy::mesh::Mesh>().init_asset::<bevy::mesh::skinning::SkinnedMeshInverseBindposes>();
        let font=app.world_mut().resource_mut::<Assets<Font>>().add(Font::from_bytes(std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"),"/../../web/public/original-ui/fonts/NotoSansCJKsc-Regular.otf")).unwrap()));
        let camera=app.world_mut().spawn((Camera2d,Camera{computed:ComputedCameraValues{target_info:Some(RenderTargetInfo{physical_size:UVec2::new(1024,768),scale_factor:1.}),..default()},viewport:Some(Viewport{physical_size:UVec2::new(1024,768),..default()}),..default()})).id();
        let body="😀e\u{301}אבג中文 a\n".repeat(15);let(mut e,d)=mounted(&body);
        assert!(e.configure_view(ComposeLayout::desktop(ComposeKind::Letter).body,12));e.set_dom_selection(0,2).unwrap();
        app.insert_resource(e).insert_resource(NativeFixture{camera,font,draft:d});
        app.add_systems(Update,paint_native_fixture).add_systems(PostUpdate,capture_layout_system.after(bevy::ui::UiSystems::PostLayout));
        for _ in 0..6{app.update();}
        let w=app.world_mut();let(tag,n,t)=w.query::<(&MailLetterEditText,&ComputedNode,&TextFont)>().single(w).unwrap();
        assert_eq!(tag.viewport,[198.,161.]);assert!(n.size().x>0.);assert_eq!(t.font_size,FontSize::Px(crate::crystal_ui::typography::CRYSTAL_DEFAULT_FONT_SIZE_PX));
        assert!(w.resource::<MailLetterEditor>().layout_ready());assert!(w.resource::<MailLetterEditor>().layout.lines.len()>1);
        assert_eq!(w.query::<&MailEditorCaret>().iter(w).count(),1);assert!(w.query::<&MailEditorSelection>().iter(w).count()>0);
        let actions=w.query::<(&NativeFixtureAction,&Node)>().iter(w).map(|(a,n)|(a.0,n.left,n.top,n.width,n.height)).collect::<Vec<_>>();
        assert!(actions.contains(&(ComposeAction::Cancel,Val::Px(135.),Val::Px(265.),Val::Px(68.),Val::Px(25.))));
    }
}

mod common_compose_candidate_repair02 {
    use super::*;
    use crate::crystal_ui::mail_compose_shared::*;

    fn mounted(body:&str)->(MailLetterEditor,mir2_ui_core::state::MailComposeDraft) {
        let mut editor=MailLetterEditor::default();
        assert!(editor.mount_external(body,9,MAIL_LETTER_BODY_RECT,11));
        (editor,draft(body))
    }
    fn base(d:&mir2_ui_core::state::MailComposeDraft)->ComposeFullRaw {
        ComposeFullRaw{to:d.recipient.clone(),subject:" untouched subject ".into(),body:d.message.clone(),
            gold_text:"0007".into(),items:vec!["UID7".into()],attachment_unique_ids:vec!["7".into()],
            stamped:true,attachment_unique_ids_present:true,stamped_present:true}
    }
    fn proof(e:&MailLetterEditor,run:u64,sequence:u64)->TextProof {
        TextProof{owner:ComposeIdentity{run,connection_generation:2,session_generation:3,owner_revision:0,
            scene_revision:4,hud_generation:5,player_object_id:6},incarnation:7,draft_epoch:8,
            draft_generation:"9".into(),editor_revision:e.revision(),focus_generation:e.focus_generation(),
            presentation_revision:10,layout_revision:e.layout_revision(),sequence}
    }
    fn apply(a:&mut CommonMailTextAdapter,e:&mut MailLetterEditor,d:&mut mir2_ui_core::state::MailComposeDraft,
        run:u64,sequence:u64,operation:TextOperation)->Option<TextIntent> {
        let p=proof(e,run,sequence);let raw=base(d);
        a.apply(&p,TextEdge{proof:p.clone(),operation},e,d,&raw)
    }

    #[test]
    fn common_ime_cursor_wire_requires_nullable_ordered_scalar_range() {
        let(e,_)=mounted("body");
        let edge=TextEdge{proof:proof(&e,1,1),operation:TextOperation::ImePreview{
            composition_id:1,text:"a😀e\u{301}".into(),cursor_utf16:None}};
        let value=serde_json::to_value(edge).unwrap();
        for cursor in [serde_json::Value::Null,serde_json::json!([0,0]),serde_json::json!([1,3]),
            serde_json::json!([3,5]),serde_json::json!([4,5]),serde_json::json!([5,5])] {
            let mut v=value.clone();v["operation"]["cursorUtf16"]=cursor.clone();
            assert!(parse_text_edge(&v.to_string()).is_some(),"valid scalar range {cursor}");
        }
        for cursor in [serde_json::json!(5),serde_json::json!([]),serde_json::json!([0]),
            serde_json::json!([0,1,3]),serde_json::json!([2,3]),serde_json::json!([1,2]),
            serde_json::json!([5,6]),serde_json::json!([3,1]),serde_json::json!([-1,0]),
            serde_json::json!([0,1.5]),serde_json::json!([0,SAFE+1])] {
            let mut v=value.clone();v["operation"]["cursorUtf16"]=cursor.clone();
            assert!(parse_text_edge(&v.to_string()).is_none(),"invalid cursor {cursor}");
        }
        let mut missing=value;missing["operation"].as_object_mut().unwrap().remove("cursorUtf16");
        assert!(parse_text_edge(&missing.to_string()).is_none(),"null is explicit, not an omitted key");
    }

    #[test]
    fn common_ime_invalid_preview_keeps_capture_and_valid_range_maps_dom_to_bytes() {
        let(mut e,mut d)=mounted("body");let mut a=CommonMailTextAdapter::default();
        let clock=e.draft_clock.generation();let revision=e.revision();
        apply(&mut a,&mut e,&mut d,1,1,TextOperation::ImeStart{composition_id:1}).unwrap();
        apply(&mut a,&mut e,&mut d,1,2,TextOperation::ImePreview{
            composition_id:1,text:"a😀e\u{301}".into(),cursor_utf16:Some([1,3])}).unwrap();
        assert_eq!(e.composition().unwrap().cursor,Some((1,5)));
        let preview=e.composition().cloned();
        for (sequence,cursor) in [(3,[3,1]),(4,[1,2]),(5,[5,6])] {
            assert!(apply(&mut a,&mut e,&mut d,1,sequence,TextOperation::ImePreview{
                composition_id:1,text:"a😀e\u{301}".into(),cursor_utf16:Some(cursor)}).is_none());
            assert_eq!(e.composition(),preview.as_ref());assert_eq!(d.message,"body");
            assert_eq!(e.revision(),revision);assert_eq!(e.draft_clock.generation(),clock);
        }
        apply(&mut a,&mut e,&mut d,1,6,TextOperation::ImePreview{
            composition_id:1,text:"中".into(),cursor_utf16:None}).unwrap();
        assert_eq!(e.composition().unwrap().cursor,None);
        apply(&mut a,&mut e,&mut d,1,7,TextOperation::ImeCommit{composition_id:1,text:"中".into()}).unwrap();
        assert_eq!(d.message,"body中");assert!(e.composition().is_none());
        assert!(apply(&mut a,&mut e,&mut d,1,8,TextOperation::ImeCommit{composition_id:1,text:"中".into()}).is_none());
        assert_eq!(d.message,"body中");
    }

    #[test]
    fn common_clipboard_request_wire_projects_null_only_for_paste_and_exact_copy_cut_selection() {
        for selected in [false,true] {
            for kind in ["copy","cut","paste"] {
                let(mut e,mut d)=mounted("a😀e\u{301}");let mut a=CommonMailTextAdapter::default();
                e.set_dom_selection(1,if selected{3}else{1}).unwrap();let raw=base(&d);
                let intent=apply(&mut a,&mut e,&mut d,1,1,TextOperation::ClipboardRequest{request_id:1,kind:kind.into()}).unwrap();
                let wire=serde_json::to_value(&intent).unwrap();
                assert_eq!(intent.base_raw,raw);assert_eq!(wire["requestId"],1);
                assert_eq!(wire["clipboardKind"],kind);assert_eq!(intent.body_mutation,None);
                if kind=="paste" {assert!(wire["selectedText"].is_null());assert_eq!(intent.selected_text,None);}
                else {let expected=if selected{"😀"}else{""};assert_eq!(wire["selectedText"],expected);assert_eq!(intent.selected_text.as_deref(),Some(expected));}
                let result=apply(&mut a,&mut e,&mut d,1,2,TextOperation::ClipboardResult{request_id:1,success:true,text:Some("中".into())}).unwrap();
                let expected=match (kind,selected){("cut",true)=>"ae\u{301}",("paste",true)=>"a中e\u{301}",("paste",false)=>"a中😀e\u{301}",_=>"a😀e\u{301}"};
                assert_eq!(d.message,expected);assert_eq!(result.base_raw,raw);
            }
        }
    }

    #[test]
    fn common_empty_cut_is_noop_while_delete_and_nonempty_cut_keep_grapheme_behavior_and_native_clock() {
        let clock=mir2_client_core::mail_compose::MailDraftClock::default();
        let mut e=MailLetterEditor::default();e.bind_draft_clock(clock.clone());let mut d=draft("😀B");e.sync(true,Some(&d.message));
        e.set_dom_selection(0,0).unwrap();let before=e.clone();let generation=clock.generation();
        e.cut_selection(&mut d);assert_eq!(d.message,"😀B");assert_eq!(e,before);assert_eq!(clock.generation(),generation);
        e.common_key("delete",false,false,&mut d).unwrap();assert_eq!(d.message,"B");assert_ne!(clock.generation(),generation);
        let generation=clock.generation();e.set_dom_selection(0,1).unwrap();assert_eq!(clock.generation(),generation);
        e.cut_selection(&mut d);assert_eq!(d.message,"");assert_ne!(clock.generation(),generation);
        let(mut e,mut d)=mounted("AB");e.set_dom_selection(0,0).unwrap();let mut a=CommonMailTextAdapter::default();
        let before=e.clone();let raw=base(&d);
        let request=apply(&mut a,&mut e,&mut d,1,1,TextOperation::ClipboardRequest{request_id:1,kind:"cut".into()}).unwrap();
        assert_eq!(request.selected_text.as_deref(),Some(""));
        let result=apply(&mut a,&mut e,&mut d,1,2,TextOperation::ClipboardResult{request_id:1,success:true,text:Some("".into())}).unwrap();
        assert_eq!(e,before);assert_eq!(base(&d),raw);assert_eq!(result.body_mutation,None);
        apply(&mut a,&mut e,&mut d,1,3,TextOperation::Key{key:"delete".into(),control:false,shift:false}).unwrap();assert_eq!(d.message,"B");
    }

    #[test]
    fn common_clipboard_ids_restart_only_on_new_checked_run_and_old_proof_cannot_consume_reused_id() {
        let(mut e,mut d)=mounted("AB");let mut a=CommonMailTextAdapter::default();
        let old=apply(&mut a,&mut e,&mut d,1,10,TextOperation::ClipboardRequest{request_id:9,kind:"paste".into()}).unwrap().proof;
        a.retire();
        assert!(apply(&mut a,&mut e,&mut d,1,11,TextOperation::ClipboardRequest{request_id:9,kind:"paste".into()}).is_none());
        assert!(apply(&mut a,&mut e,&mut d,1,10,TextOperation::ClipboardRequest{request_id:10,kind:"paste".into()}).is_none());
        assert_eq!(a.highwater(),11);
        let mut changed=proof(&e,1,12);changed.owner.owner_revision=1;let raw=base(&d);
        assert!(a.apply(&changed,TextEdge{proof:changed.clone(),operation:TextOperation::ClipboardRequest{request_id:9,kind:"paste".into()}},&mut e,&mut d,&raw).is_none(),"same run capture-owner change retains ID highwater");
        a.retire();
        let new=apply(&mut a,&mut e,&mut d,2,1,TextOperation::ClipboardRequest{request_id:1,kind:"paste".into()}).unwrap();
        assert_eq!(new.selected_text,None);assert_eq!(a.highwater(),1);
        let current=proof(&e,2,2);let raw=base(&d);let mut old=old;old.sequence=100;
        assert!(a.apply(&current,TextEdge{proof:old.clone(),operation:TextOperation::ClipboardResult{request_id:1,success:true,text:Some("stale".into())}},&mut e,&mut d,&raw).is_none());
        assert!(a.apply(&old,TextEdge{proof:old.clone(),operation:TextOperation::ClipboardResult{request_id:1,success:true,text:Some("rollback".into())}},&mut e,&mut d,&raw).is_none(),"even a self-consistent older run cannot reset the namespace");
        let mut invalid=proof(&e,3,1);invalid.owner.scene_revision=0;
        assert!(a.apply(&invalid,TextEdge{proof:invalid.clone(),operation:TextOperation::ClipboardRequest{request_id:1,kind:"paste".into()}},&mut e,&mut d,&raw).is_none());
        assert_eq!(a.highwater(),1);assert_eq!(d.message,"AB");
        apply(&mut a,&mut e,&mut d,2,2,TextOperation::ClipboardResult{request_id:1,success:true,text:Some("中".into())}).unwrap();
        assert_eq!(d.message,"AB中");
    }

    #[test]
    fn common_composition_ids_restart_only_on_new_run_and_old_commit_keeps_new_preview_live() {
        let(mut e,mut d)=mounted("AB");let mut a=CommonMailTextAdapter::default();
        let old=apply(&mut a,&mut e,&mut d,1,10,TextOperation::ImeStart{composition_id:9}).unwrap().proof;
        apply(&mut a,&mut e,&mut d,1,11,TextOperation::ImePreview{composition_id:9,text:"旧".into(),cursor_utf16:Some([1,1])}).unwrap();
        a.retire();e.clear_composition();
        assert!(apply(&mut a,&mut e,&mut d,1,12,TextOperation::ImeStart{composition_id:9}).is_none());
        apply(&mut a,&mut e,&mut d,1,13,TextOperation::ImeStart{composition_id:10}).unwrap();
        let mut changed=proof(&e,1,14);changed.owner.owner_revision=1;let raw=base(&d);
        assert!(a.apply(&changed,TextEdge{proof:changed.clone(),operation:TextOperation::ImeStart{composition_id:9}},&mut e,&mut d,&raw).is_none());
        apply(&mut a,&mut e,&mut d,2,1,TextOperation::ImeStart{composition_id:1}).unwrap();assert!(e.composition().is_none());
        apply(&mut a,&mut e,&mut d,2,2,TextOperation::ImePreview{composition_id:1,text:"新".into(),cursor_utf16:Some([1,1])}).unwrap();
        let preview=e.composition().cloned();let current=proof(&e,2,3);let raw=base(&d);let mut old=old;old.sequence=100;
        assert!(a.apply(&current,TextEdge{proof:old.clone(),operation:TextOperation::ImeCommit{composition_id:1,text:"stale".into()}},&mut e,&mut d,&raw).is_none());
        assert!(a.apply(&old,TextEdge{proof:old.clone(),operation:TextOperation::ImeCommit{composition_id:1,text:"rollback".into()}},&mut e,&mut d,&raw).is_none());
        assert_eq!(a.highwater(),2);assert_eq!(e.composition(),preview.as_ref());assert_eq!(d.message,"AB");
        apply(&mut a,&mut e,&mut d,2,3,TextOperation::ImeCommit{composition_id:1,text:"新".into()}).unwrap();assert_eq!(d.message,"AB新");
        assert!(apply(&mut a,&mut e,&mut d,2,4,TextOperation::ImeCommit{composition_id:1,text:"duplicate".into()}).is_none());assert_eq!(d.message,"AB新");
    }
}