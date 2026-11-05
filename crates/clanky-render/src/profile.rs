//! Opt-in frame-phase counters. No text, keys, paths from documents or pixel data.
use std::{cell::RefCell,collections::BTreeMap,fs::{File,OpenOptions},io::Write,sync::OnceLock,time::Instant};
fn enabled()->bool{static ENABLED:OnceLock<bool>=OnceLock::new();*ENABLED.get_or_init(||std::env::var_os("CLANKY_FRAME_TRACE").is_some())}
thread_local!{static TIMES:RefCell<BTreeMap<&'static str,(u64,u128)>>=RefCell::new(BTreeMap::new());static OUTPUT:RefCell<Option<File>>=RefCell::new(None);static FRAME:RefCell<u64>=const{RefCell::new(0)};}
pub(crate) struct Span{label:&'static str,start:Instant}
pub(crate) fn span(label:&'static str)->Option<Span>{enabled().then(||Span{label,start:Instant::now()})}
impl Drop for Span{fn drop(&mut self){let elapsed=self.start.elapsed().as_nanos();TIMES.with(|times|{let mut times=times.borrow_mut();let entry=times.entry(self.label).or_default();entry.0+=1;entry.1+=elapsed;});}}
pub(crate) fn flush(){if !enabled(){return;}let times=TIMES.with(|t|std::mem::take(&mut *t.borrow_mut()));let frame=FRAME.with(|f|{*f.borrow_mut()+=1;*f.borrow()});OUTPUT.with(|out|{let mut out=out.borrow_mut();if out.is_none(){*out=std::env::var_os("CLANKY_FRAME_TRACE").and_then(|path|OpenOptions::new().create(true).append(true).open(std::path::PathBuf::from(path).with_extension("gpu.csv")).ok());}if let Some(file)=out.as_mut(){for (label,(count,nanos))in times{let _=writeln!(file,"{frame},{label},{count},{nanos}");}}});}
