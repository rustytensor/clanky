//! Settings rows: a label with its description on the left and its control on the right (a
//! switch, segments or a text field), as the approved Settings rows (68.84 px apart).
use super::{scene::Paint,settings_frame::{self,Action,Geometry,Hit}};
use crate::{look::PixelRect,themes::Theme};

fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w:w.max(0.0),h:h.max(0.0)}}
pub(super) const ROW:f32=68.84375;

/// The field being edited in place: its key, text, caret (a byte offset) and whether all of it is selected.
#[derive(Clone,Copy)]
pub(super) struct Editing<'a>{pub key:&'a str,pub index:usize,pub text:&'a str,pub caret:usize,pub all:bool,
    /// Why the typed value was not saved, in the row's own words (shown in the row, the field marked).
    pub problem:Option<&'a str>}

pub(super) enum Control{
    /// A boolean: the whole row flips it.
    Switch(&'static str,bool),
    /// Labelled words (label, config key, value) and the value now saved.
    Segments(Vec<(&'static str,&'static str,&'static str)>,&'static str),
    /// A text field: config key, the stored value as config.xml writes it, width in page px, mono text.
    Field(&'static str,String,f32,bool),
}
pub(super) struct Row{pub label:String,pub description:String,pub control:Control}
pub(super) fn row(label:&str,description:&str,control:Control)->Row{Row{label:label.into(),description:description.into(),control}}

/// A group heading at page y (mockup .set h4: 15 px semibold) with a hairline under it; returns
/// the y where its rows start.
pub(super) fn heading(p:&mut Paint<'_>,g:&Geometry,t:&Theme,y:f32,label:&str)->f32{
    heading_noted(p,g,t,y,label,"")
}
/// As `heading`, with a muted note at the right end of its line.
pub(super) fn heading_noted(p:&mut Paint<'_>,g:&Geometry,t:&Theme,y:f32,label:&str,note:&str)->f32{
    let s=g.scale;let c=&t.colors;
    if !settings_frame::fits(g,y,28.0){return y+28.0;}
    let w=p.width(label,15.0*s,600)/s+2.0;
    p.text(g.local(229.0,y,w,20.0),label,15.0*s,c.foreground,600,false);
    if !note.is_empty(){let nw=(p.width(note,13.0*s,400)/s+1.0).min(522.0-w-16.0);p.text(g.local(751.0-nw,y,nw,20.0),note,13.0*s,c.muted_foreground,400,false);}
    p.shape(g.local(229.0,y+27.0,522.0,1.0),0.0,c.border,None);
    y+28.0
}

/// A 32 x 18 switch at the control rect's left, centred in it.
pub(super) fn switch(p:&mut Paint<'_>,control:PixelRect,on:bool,t:&Theme,s:f32){
    let c=&t.colors;let sw=r(control.x,control.y+7.0*s,32.0*s,18.0*s);
    // Off as the mockup's chip switch: a muted track with a visible edge and a dim knob.
    if on{p.shape(sw,9.0*s,c.primary,None);}else{p.shape(sw,9.0*s,c.muted,Some(c.ring));}
    p.shape(r(sw.x+if on{16.0*s}else{2.0*s},sw.y+2.0*s,14.0*s,14.0*s),7.0*s,if on{c.primary_foreground}else{c.muted_foreground},None);
}

/// Segments in a pill (as Appearance › Layout); each one a hit with its (key, value).
pub(super) fn segments(p:&mut Paint<'_>,control:PixelRect,options:&[(&'static str,&'static str,&'static str)],selected:&str,t:&Theme,s:f32,hits:&mut Vec<Hit>){
    let c=&t.colors;
    p.shape(control,8.0*s,c.card,Some(c.border));
    let mut x=control.x+3.0*s;
    for (name,key,value) in options{
        let w=p.width(name,14.0*s,500)+26.0*s;
        let b=r(x,control.y+3.0*s,w,control.h-6.0*s);let on=selected==*value;
        if on{p.shape(b,6.0*s,c.pressed,None);}else if b.contains(p.pointer.0,p.pointer.1){p.shape(b,6.0*s,c.accent,None);}
        let tw=p.width(name,14.0*s,500);
        p.text(r(b.x+(b.w-tw)/2.0,b.y,tw,b.h),name,14.0*s,if on{c.foreground}else{c.muted_foreground},500,false);
        hits.push(Hit{rect:b,action:Action::Choice(key,value)});x+=w;
    }
}
pub(super) fn segments_width(p:&mut Paint<'_>,options:&[(&'static str,&'static str,&'static str)],s:f32)->f32{
    options.iter().map(|(name,_,_)|p.width(name,14.0*s,500)/s+26.0).sum::<f32>()+6.0
}

/// A text field: its stored value, or while edited the text with a caret (the selection, when
/// everything is selected, shown as a highlight behind it).
#[allow(clippy::too_many_arguments)]
pub(super) fn field(p:&mut Paint<'_>,rect:PixelRect,value:&str,mono:bool,editing:Option<Editing<'_>>,t:&Theme,s:f32,placeholder:&str){
    let c=&t.colors;
    let focused=editing.is_some();let ring=if editing.is_some_and(|e|e.problem.is_some()){c.err}else{c.ring};
    // A filled field, as a GNOME entry: a soft fill, an edge only while it has the focus.
    if focused{p.shape(rect,8.0*s,c.background,Some(ring));}else{p.shape(rect,8.0*s,c.muted,None);}
    if focused{p.shape(r(rect.x-s,rect.y-s,rect.w+2.0*s,rect.h+2.0*s),9.0*s,[0;4],Some(ring));}
    let inner=r(rect.x+10.0*s,rect.y,rect.w-20.0*s,rect.h);
    let measure=|p:&mut Paint<'_>,text:&str|if mono{p.mono_width(text,14.0*s,400)}else{p.width(text,14.0*s,400)};
    match editing{
        Some(edit)=>{
            let text=edit.text;let caret=edit.caret.min(text.len());
            // Show the end of a long value around the caret.
            let mut start=0;while start<caret && measure(p,&text[start..caret])>inner.w-2.0*s{start+=text[start..].chars().next().map_or(1,char::len_utf8);}
            let shown=&text[start..];
            if edit.all && !text.is_empty(){let w=measure(p,shown).min(inner.w);let mut sel=c.ring;sel[3]=110;p.shape(r(inner.x-2.0*s,rect.y+7.0*s,w+4.0*s,18.0*s),3.0*s,sel,None);}
            p.text(inner,shown,14.0*s,c.foreground,400,mono);
            if !edit.all{let x=inner.x+measure(p,&text[start..caret]);p.shape(r(x,rect.y+8.0*s,s,16.0*s),0.0,c.foreground,None);}
        }
        None=>{
            if value.is_empty(){p.text(inner,placeholder,14.0*s,crate::calm::scene_faint(c),400,false);}
            else{p.text(inner,value,14.0*s,c.foreground,400,mono);}
        }
    }
}

/// Rows from page y (already scrolled through `g`); returns the y after the last one.
pub(super) fn rows(p:&mut Paint<'_>,g:&Geometry,t:&Theme,y:f32,rows:&[Row],editing:Option<Editing<'_>>,hits:&mut Vec<Hit>)->f32{
    rows_then(p,g,t,y,rows,editing,hits,false)
}
/// As `rows`, with a divider under the last row too when more rows follow it.
#[allow(clippy::too_many_arguments)]
pub(super) fn rows_then(p:&mut Paint<'_>,g:&Geometry,t:&Theme,mut y:f32,rows:&[Row],editing:Option<Editing<'_>>,hits:&mut Vec<Hit>,more:bool)->f32{
    let s=g.scale;
    for (i,item) in rows.iter().enumerate(){
        let last=i+1==rows.len() && !more;
        if !settings_frame::row_fits(g,y){y+=ROW;continue;}
        let width=match &item.control{Control::Switch(..)=>32.0,Control::Segments(options,_)=>segments_width(p,options,s),Control::Field(_,_,w,_)=>*w};
        let problem=match &item.control{Control::Field(key,..)=>editing.filter(|e|e.key==*key).and_then(|e|e.problem),_=>None};
        let control=settings_frame::row_noted(p,g,t,y,width,&item.label,&item.description,problem,last);
        match &item.control{
            Control::Switch(key,on)=>{switch(p,control,*on,t,s);hits.push(Hit{rect:g.local(229.0,y,522.0,if last{ROW-1.0}else{ROW}),action:Action::Toggle(key,!on)});}
            Control::Segments(options,selected)=>segments(p,control,options,selected,t,s,hits),
            Control::Field(key,value,_,mono)=>{
                let control=settings_frame::level(g,control);
                let edit=editing.filter(|e|e.key==*key);
                field(p,control,value,*mono,edit,t,s,"");
                hits.push(Hit{rect:control,action:Action::Edit(key)});
            }
        }
        y+=ROW;
    }
    y
}

/// An outlined button (14 px text, 32 px tall, radius 8) at page x, y; returns its rect.
pub(super) fn button(p:&mut Paint<'_>,g:&Geometry,t:&Theme,x:f32,y:f32,label:&str)->PixelRect{
    let s=g.scale;let c=&t.colors;let width=p.width(label,14.0*s,500)/s+24.0;let b=g.local(x,y,width,32.0);
    let fill=if b.contains(p.pointer.0,p.pointer.1){if p.pressed{c.pressed}else{c.accent}}else{c.background};p.shape(b,8.0*s,fill,Some(c.input));
    let w=p.width(label,14.0*s,500);p.text(r(b.x+(b.w-w)/2.0,b.y,w,b.h),label,14.0*s,c.foreground,500,false);
    b
}

/// A colour swatch (a square filled with the colour) with a thin ring.
pub(super) fn swatch(p:&mut Paint<'_>,rect:PixelRect,color:crate::themes::Color,t:&Theme,s:f32){
    p.shape(rect,5.0*s,color,Some(t.colors.input));
}

/// Hits scrolled out of the page body are dropped; partly visible ones are trimmed.
pub(super) fn clip_hits(hits:&mut Vec<Hit>,from:usize,body:PixelRect){
    let mut kept=hits.split_off(from);
    kept.retain_mut(|hit|{let top=hit.rect.y.max(body.y);let bottom=(hit.rect.y+hit.rect.h).min(body.y+body.h);hit.rect.y=top;hit.rect.h=(bottom-top).max(0.0);hit.rect.h>0.0});
    hits.extend(kept);
}
