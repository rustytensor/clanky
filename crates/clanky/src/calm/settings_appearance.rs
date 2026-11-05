//! Appearance contents and sheet-style edit popovers; storage stays in ThemeStore.
use super::{scene::Paint,settings_frame::{Action,Geometry,Hit}};
use crate::{config::{Config,ThemePreference},look::PixelRect,theme_store::ThemeStore,themes::{Theme,Color,COLOR_TOKENS}};
use clanky_render::{Fill,Shape,Layer,TextRun,TextMetrics};
use winit::keyboard::{Key,NamedKey};
fn r(x:f32,y:f32,w:f32,h:f32)->PixelRect{PixelRect{x,y,w,h}}
pub(super) fn hex(c:Color)->String{if c[3]==255{format!("#{:02x}{:02x}{:02x}",c[0],c[1],c[2])}else{format!("#{:02x}{:02x}{:02x}{:02x}",c[0],c[1],c[2],c[3])}}
fn label(token:&str)->String{let text=token.replace('_'," ");let mut chars=text.chars();chars.next().map(|c|c.to_uppercase().collect::<String>()+chars.as_str()).unwrap_or_default()}
fn button(p:&mut Paint<'_>,rect:PixelRect,text:&str,t:&Theme,s:f32,pointer:(f64,f64),pressed:bool,outline:bool){
    let fill=if rect.contains(pointer.0,pointer.1){if pressed{t.colors.pressed}else{t.colors.accent}}else{t.colors.background};
    if outline || rect.contains(pointer.0,pointer.1){p.shape(rect,6.0*s,fill,outline.then_some(t.colors.input));}
    let w=p.width(text,13.0*s,500);p.text(r(rect.x+(rect.w-w)/2.0,rect.y,w,rect.h),text,13.0*s,t.colors.foreground,500,false);
}
pub(super) fn page(window:[u32;2],g:&Geometry,t:&Theme,cfg:&Config,store:&ThemeStore,system_light:bool,scroll:f32,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->Result<(Vec<Layer>,Vec<Hit>,f32),String>{
    let s=g.scale;let mut p=Paint::empty(t,measure,s,window,pointer,pressed);let mut hits=vec![];
    let rect=|x,y,w,h|g.local(x,y-scroll,w,h);
    p.out.scene.layers.push(Layer::Clip(Some(g.body.array())));
    let modes=[("Light","light"),("Dark","dark"),("System","system")];let widths=modes.map(|(name,_)|p.width(name,13.0*s,500)/s+24.0);let total=widths.iter().sum::<f32>()+10.0;
    p.text(rect(229.0,79.0,522.0-total-24.0,18.0),"Mode",14.0*s,t.colors.foreground,500,false);
    p.text(rect(229.0,100.0,522.0-total-24.0,18.84375),"Follow the system, or pick one.",13.0*s,t.colors.muted_foreground,400,false);
    let mut x=751.0-total;p.shape(rect(x,82.921875,total,32.0),8.0*s,t.colors.muted,None);x+=3.0;
    let selected=match cfg.theme{ThemePreference::Light=>"light",ThemePreference::Dark=>"dark",ThemePreference::System=>"system"};
    for ((name,value),w) in modes.into_iter().zip(widths){let b=rect(x,85.921875,w,26.0);if selected==value{p.shape(b,6.0*s,t.colors.background,None);}else if b.contains(pointer.0,pointer.1){p.shape(b,6.0*s,t.colors.accent,None);}
        let width=p.width(name,13.0*s,500);p.text(r(b.x+(b.w-width)/2.0,b.y,width,b.h),name,13.0*s,if selected==value{t.colors.foreground}else{t.colors.muted_foreground},500,false);hits.push(Hit{rect:b,action:Action::Mode(value)});x+=w+2.0;
    }
    p.shape(rect(229.0,132.84375,522.0,1.0),0.0,t.colors.border,None);
    let appearance=t.appearance;let entries=store.picker(Some(appearance))?;let selected_id=cfg.selected_theme_id(system_light);
    p.text(rect(229.0,153.84375,522.0,16.0),if appearance==crate::themes::Appearance::Dark{"Dark theme"}else{"Light theme"},12.0*s,t.colors.muted_foreground,500,false);
    for(i,entry)in entries.iter().enumerate(){let x=229.0+(i%4) as f32*133.5;let y=185.84375+(i/4) as f32*100.0;let pv=rect(x,y,121.5,64.0);
        let theme=store.get(&entry.id).ok();let bg=entry.swatches.background;let side=theme.as_ref().map(|theme|theme.colors.sidebar).unwrap_or(bg);
        p.shape(pv,8.0*s,bg,Some(t.colors.border));
        let fill=entry.window_gradient.as_ref().map(|stops|Fill::VerticalStops(stops.clone())).unwrap_or(Fill::Solid(side));
        for(area,radius)in[(rect(x+1.0,y+1.0,22.0,62.0),7.0*s),(rect(x+8.0,y+1.0,15.0,62.0),0.0)]{p.out.scene.layers.push(Layer::Shape(Shape{rect:area.array(),radius,fill:fill.clone(),border:None,shadow:None}));}
        let mut secondary=entry.swatches.foreground;secondary[3]=115;
        for(n,(width,color))in[(57.05,entry.swatches.foreground),(40.75,secondary),(24.45,entry.swatches.agent)].into_iter().enumerate(){p.shape(rect(x+31.0,y+9.0+n as f32*10.0,width,5.0),3.0*s,color,None);}
        let on=entry.id==selected_id;if on{p.out.scene.layers.push(Layer::Shape(Shape{rect:rect(x-4.0,y-4.0,129.5,72.0).array(),radius:12.0*s,fill:Fill::Solid([0;4]),border:Some((2.0*s,t.colors.ring)),shadow:None}));}
        p.text(rect(x,y+71.0,if on{101.5}else{121.5},17.0),&entry.name,13.0*s,t.colors.foreground,500,false);
        if on{let name_width=(p.width(&entry.name,13.0*s,500)/s).min(101.5);super::settings_frame::icon(&mut p,"check",rect(x+name_width+6.0,y+72.5,14.0,14.0),t.colors.foreground,s*0.875);}
        hits.push(Hit{rect:rect(x,y,121.5,88.0),action:Action::Theme(entry.id.clone())});
    }
    let rows=entries.len().div_ceil(4).max(1) as f32;let end=173.84375+117.0+(rows-1.0)*100.0;
    p.shape(rect(229.0,end-1.0,522.0,1.0),0.0,t.colors.border,None);
    p.text(rect(229.0,end+20.0,522.0,16.0),&format!("Colours in {}",t.name),12.0*s,t.colors.muted_foreground,500,false);
    let base=store.get(selected_id).ok();let mut y=end+40.0;
    let tokens=["sidebar","background","agent"].into_iter().chain(COLOR_TOKENS.iter().copied().filter(|token|!["sidebar","background","agent"].contains(token)));
    for token in tokens{let Some(color)=t.colors.get(token)else{continue;};let row=rect(229.0,y,522.0,40.0);
        p.shape(rect(229.0,y+10.0,20.0,20.0),5.0*s,color,Some(t.colors.input));let name=if token=="agent"{"Agent accent".into()}else{label(token)};p.text(rect(259.0,y,260.0,40.0),&name,14.0*s,t.colors.foreground,400,false);
        let changed=cfg.colors.get(token).is_some_and(|value|base.as_ref().and_then(|b|b.colors.get(token)).as_ref()!=Some(value));
        let value=hex(color);let width=p.mono_width(&value,13.0*s,400);let right=if changed{679.0}else{751.0};p.text(r(g.panel.x+right*s-width,g.panel.y+(y-scroll)*s,width,40.0*s),&value,13.0*s,t.colors.muted_foreground,400,true);
        hits.push(Hit{rect:row,action:Action::Color(token)});
        if changed{let b=rect(689.0,y+6.0,62.0,28.0);button(&mut p,b,"Reset",t,s,pointer,pressed,false);hits.push(Hit{rect:b,action:Action::ResetColor(token)});}
        p.shape(rect(229.0,y+39.0,522.0,1.0),0.0,t.colors.border,None);y+=40.0;
    }
    let mut x=229.0;
    for(name,action)in[("Save as new theme",Action::SaveTheme),("Import",Action::ImportTheme),("Export",Action::ExportTheme)]{let width=p.width(name,13.0*s,500)/s+20.0;let b=rect(x,y+16.0,width,28.0);button(&mut p,b,name,t,s,pointer,pressed,true);hits.push(Hit{rect:b,action});x+=width+8.0;}
    hits.retain_mut(|hit|{let top=hit.rect.y.max(g.body.y);let bottom=(hit.rect.y+hit.rect.h).min(g.body.y+g.body.h);hit.rect.y=top;hit.rect.h=(bottom-top).max(0.0);hit.rect.h>0.0});
    p.out.scene.layers.push(Layer::Clip(None));Ok((p.out.scene.layers,hits,(y+68.0-543.0).max(0.0)))
}
#[derive(Clone)]
pub(super) enum EditKind{Color(&'static str),Save,Import,Export}
pub(super) struct Editor{pub kind:EditKind,pub values:Vec<String>,pub focus:usize,caret:usize,all:bool}
impl Editor{
    pub fn new(kind:EditKind,values:Vec<String>)->Self{let caret=values[0].len();Self{kind,values,focus:0,caret,all:true}}
    pub fn field(&mut self,index:usize){self.focus=index;self.caret=self.values[index].len();self.all=true;}
    pub fn select_all(&mut self){self.all=true;}
    pub fn insert(&mut self,text:&str){let Some(value)=self.values.get_mut(self.focus)else{return;};let text:String=text.chars().filter(|c|!c.is_control()).take(4096).collect();if self.all{value.clear();self.caret=0;self.all=false;}if value.len()+text.len()>4096{return;}value.insert_str(self.caret,&text);self.caret+=text.len();}
    pub fn key(&mut self,key:&Key,text:Option<&str>){
        if *key==Key::Named(NamedKey::Tab){self.focus=(self.focus+1)%(self.values.len()+2);if let Some(value)=self.values.get(self.focus){self.caret=value.len();self.all=true;}return;}
        let Some(value)=self.values.get_mut(self.focus)else{return;};
        match key{
            Key::Named(NamedKey::Backspace)|Key::Named(NamedKey::Delete)=>{if self.all{value.clear();self.caret=0;self.all=false;}else if *key==Key::Named(NamedKey::Backspace){if let Some((i,_))=value[..self.caret].char_indices().last(){value.drain(i..self.caret);self.caret=i;}}else if let Some(c)=value[self.caret..].chars().next(){value.drain(self.caret..self.caret+c.len_utf8());}},
            Key::Named(NamedKey::ArrowLeft)=>{self.caret=value[..self.caret].char_indices().last().map_or(0,|(i,_)|i);self.all=false;},
            Key::Named(NamedKey::ArrowRight)=>{self.caret+=value[self.caret..].chars().next().map_or(0,char::len_utf8);self.all=false;},
            Key::Named(NamedKey::Home)=>{self.caret=0;self.all=false;},Key::Named(NamedKey::End)=>{self.caret=value.len();self.all=false;},
            _=>if let Some(text)=text{self.insert(text);},
        }
    }
    pub fn draw(&self,window:[u32;2],g:&Geometry,t:&Theme,error:Option<&str>,pointer:(f64,f64),pressed:bool,measure:&mut dyn FnMut(&TextRun)->TextMetrics)->(Vec<Layer>,Vec<Hit>){
        let s=g.scale;let mut p=Paint::empty(t,measure,s,window,pointer,pressed);let mut hits=vec![];
        p.shape(g.panel,12.0*s,[0,0,0,100],None);let box_rect=g.local(220.0,140.0,548.0,330.0);p.shape(box_rect,12.0*s,t.colors.card,Some(t.colors.border));
        let (title,labels)=match self.kind{EditKind::Color(_)=>("Choose colour",vec!["Hex colour"]),EditKind::Save=>("Save as new theme",vec!["Name","File name"]),EditKind::Import=>("Import theme",vec!["Source file","File name"]),EditKind::Export=>("Export theme",vec!["Destination file"])};
        p.text(g.local(240.0,156.0,500.0,32.0),title,16.0*s,t.colors.foreground,600,false);
        for(i,(name,value))in labels.iter().zip(&self.values).enumerate(){let y=202.0+i as f32*68.0;p.text(g.local(240.0,y-20.0,500.0,18.0),name,13.0*s,t.colors.muted_foreground,400,false);let input=g.local(240.0,y,508.0,32.0);p.shape(input,8.0*s,t.colors.background,Some(if self.focus==i{t.colors.ring}else{t.colors.input}));
            let caret=if self.focus==i{self.caret}else{value.len()};let mut start=0;while p.mono_width(&value[start..caret],13.0*s,400)>488.0*s{start+=value[start..].chars().next().unwrap().len_utf8();}
            p.text(r(input.x+10.0*s,input.y,input.w-20.0*s,input.h),&value[start..],13.0*s,t.colors.foreground,400,true);
            if self.focus==i{let x=input.x+10.0*s+p.mono_width(&value[start..caret],13.0*s,400);p.shape(r(x,input.y+8.0*s,s,16.0*s),0.0,t.colors.foreground,None);}
            hits.push(Hit{rect:input,action:Action::Field(i)});
        }
        if let EditKind::Color(_)=self.kind{for(i,color)in t.ansi.iter().enumerate(){let rect=g.local(240.0+i as f32*31.0,256.0,26.0,26.0);p.shape(rect,5.0*s,*color,Some(t.colors.input));hits.push(Hit{rect,action:Action::PickColor(hex(*color))});}}
        if let Some(error)=error{p.text(g.local(240.0,350.0,508.0,42.0),error,13.0*s,t.colors.err,400,false);}
        for (x,name,action)in[(568.0,"Cancel",Action::Cancel),(664.0,"Apply",Action::Confirm)]{let rect=g.local(x,416.0,84.0,32.0);button(&mut p,rect,name,t,s,pointer,pressed,true);hits.push(Hit{rect,action});}
        (p.out.scene.layers,hits)
    }
}
