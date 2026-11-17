#!/usr/bin/env python3
"""First M12 click check: private real window, actual pointer, server workspace.

Quiet layout (design 2026-11-13, Marc's Y1 of 2026-11-14). Window 1240 x 760, scale 1. Where
a control's place depends on text width it is found on screen, as the Q1 gate does; everything else is fixed
from the mockup CSS [M] and the Q1 gate [Q1]:
  title bar 38 px [M .win]; tools toggle at x 10..36 [M .titlebar padding 10, .ico 26]; the first tab's pill
    starts at x 48 [M .tabs margin-left 8 after the toggle and a 4 px gap], its state dot at x 59..65
    [M .tab padding 11, .dot 6]; the "+" is the last mark right of the tabs and left of x 862 [M .plus, Q1 T1];
    the search box is 318 wide (Marc's Y1 of 2026-11-14, was 260 [M .search]) and still ends at the right next to
    the gear [M], so it starts at 1240 - 10 - 34 - 318 = 878, or left of it by any gap before the gear (padding 10 [M], a 34 px header button [R1,
    Ptyxis]); the "+" scan stops at 878 - 16 = 862, so a gap of up to 16 px between box and gear never brings the
    box's edge or its icon into it. Tabs are titled by their folder ("m12-click-" and 8 random letters, wider or
    narrower by letter), so with three tabs the "+" lands anywhere from about x 600 to 710: an old bound of 620
    picked the third tab's title instead of "+" whenever the random name was wide. A click at x 1070
    lands in it either way [M .search 940..1200, Y1 882..1200]; the settings gear x 1196..1230 (R1's 34 px button
    after the 10 px padding; was 1204..1230 [M, Q1 T3]), so a click at x 1217 opens Settings either way.
  New tab = a click on "+"; a session = a click on its tab; Split = a click on the search box, then
    "Split right" and Enter [Q1 T3] (the mockup's title bar has no Split button).
  Agent access: the tools panel footer's "Agents" segment, then the pane's "off" row [Q1 T4] (R3: today's
    right column moved into the tools panel).
  Settings: the gear opens it; its 780 x 600 frame is unchanged [Q1] (close (980,109)); the nav lists General,
    Appearance, Terminal, Editor, Agents, History, Keys [C1] in the old rows, 34 px apart (General (270,154),
    Appearance (270,188)). Appearance starts with the Layout group [Q1 E1]: its "Vertical" segment writes tab_style
    "vertical", "Horizontal" writes it back (in place of the old Light mode segment, which the redesign removed).
    Font size is a field on the Appearance page, below the theme lists [C1]: its row is found by the label "Font size"
    (the page scrolled with the wheel), its value by the stored text on that line; a click on the value, Ctrl+A,
    "16", Enter writes font_size 16. The window starts at R1's default (Marc's R1 2026-11-14: terminal text 11 pt =
    11 x 4 / 3 = 14.67 px; the old default was 13), and 16 px is 12 pt, one point up (the old + step was 13 -> 14).
    Animations is a switch row on the General page [C1], found by its label; a click on the label flips it.

With CLANKY_BIN (clanky-gpui, G2) the window chrome's clicks run, each control found by its label in the window's
control record (scripts/window-chrome/RECORD.md) and clicked with the real pointer at its box's centre: the tools
panel's "Agents" segment, then the pane's "Agent access to ..." switch turns access on; "New tab" makes 2 sessions;
a click on the first tab selects session 0; two splits make 2 then 3 panes; a click on the second tab selects
session 1. The server, the window and the owner helper (clanky-agent-gate from CARGO_TARGET_DIR's release or debug
folder) are as above. Moved: the splits come from Ctrl+Shift+D, as the palette that clicks them comes in G3; the
Settings and Post-AI steps move to G4.
"""
import json
import os
from pathlib import Path
import importlib.util as _config_xml_util
_config_xml_spec = _config_xml_util.spec_from_file_location('clanky_config_xml', Path(__file__).with_name('clanky_config_xml.py'))
config_xml = _config_xml_util.module_from_spec(_config_xml_spec)
_config_xml_spec.loader.exec_module(config_xml)  # config.xml writer and reader shared by the gate harnesses
import select
import subprocess
import sys
import tempfile
import time
from PIL import ImageChops, ImageGrab


def require(ok, label):
    if not ok: raise RuntimeError(label)


def until(fn, label, seconds=10):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        value=fn()
        if value:return value
        time.sleep(.05)
    raise RuntimeError(label)


def ocr_words(im, box, tmp, scale=3, psm=11):
    """Words in box as (text, x0, y0, x1, y1) in window pixels, lowercased (the Q1 gate's reader)."""
    from PIL import Image, ImageOps
    crop=im.crop(box).convert('L')
    if sum(crop.getdata())/max(1,crop.width*crop.height)<128:crop=ImageOps.invert(crop)
    crop=crop.resize((crop.width*scale,crop.height*scale),Image.LANCZOS);path=Path(tmp)/'ocr.png';crop.save(path)
    out=subprocess.run(['tesseract',str(path),'-','--psm',str(psm),'tsv'],capture_output=True,text=True,timeout=60).stdout
    words=[]
    for line in out.splitlines()[1:]:
        f=line.split('\t')
        if len(f)==12 and f[11].strip():
            x,y,w,h=(int(v) for v in f[6:10])
            words.append((f[11].strip().lower(),box[0]+x/scale,box[1]+y/scale,box[0]+(x+w)/scale,box[1]+(y+h)/scale))
    return words


def word(words,text,pick=min):
    hits=[w for w in words if w[0]==text]
    return pick(hits,key=lambda w:w[1]) if hits else None


def centre(w):return (int((w[1]+w[3])/2),int((w[2]+w[4])/2))


def marks(im,x0,x1,y0=6,y1=32):
    """Horizontal clusters (xa, xb) of title-bar columns with a pixel more than 40 from the bar colour."""
    px=im.load();bg=px[600,3]
    cols=[x for x in range(x0,x1) if any(max(abs(a-b) for a,b in zip(px[x,y],bg))>40 for y in range(y0,y1))]
    out=[]
    for x in cols:
        if out and x-out[-1][1]<=3:out[-1][1]=x
        else:out.append([x,x])
    return [tuple(c) for c in out]


def stop(p):
    if p.poll() is None:p.terminate()
    try:p.wait(timeout=5)
    except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)


def record_controls(path):
    """The main window's controls from the control record's latest main-window screen line."""
    lines=[json.loads(l) for l in Path(path).read_text().splitlines() if l.strip()] if Path(path).exists() else []
    screens=[l for l in lines if l.get('record')=='screen' and l.get('screen')=='main-window']
    return screens[-1]['controls'] if screens else []


def record_centre(path,pick,label):
    """The centre of the control the record lists for pick(control), waiting for it to be drawn."""
    def find():
        hits=[c for c in record_controls(path) if pick(c)]
        return min(hits,key=lambda c:c['box'][0]) if hits else None
    c=until(find,label,5);x,y,w,h=c['box'];return x+w//2,y+h//2


def main_gpui(gpui):
    """The window chrome's clicks on clanky-gpui (see the docstring)."""
    target=Path(os.environ['CARGO_TARGET_DIR'])
    helper=next((d/'clanky-agent-gate' for d in (target/'release',target/'debug') if (d/'clanky-agent-gate').exists()),None)
    require(helper,f'the owner helper clanky-agent-gate is built in {target}/release or debug')
    with tempfile.TemporaryDirectory(prefix='m12-click-',dir=target) as tmp:
        work=Path(tmp)
        env={k:v for k,v in os.environ.items() if not any(x in k.upper() for x in ('KEY','TOKEN','SECRET','PASSWORD','PASSWD'))}
        for key in ('WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS'):env.pop(key,None)
        for name in ('home','config','state','runtime'):(work/name).mkdir(mode=0o700)
        rec=work/'record.jsonl'
        env.update(HOME=str(work/'home'),XDG_CONFIG_HOME=str(work/'config'),XDG_STATE_HOME=str(work/'state'),
                   XDG_RUNTIME_DIR=str(work/'runtime'),SHELL='/bin/sh',CLANKY_CONTROL_RECORD=str(rec))
        config=work/'config.xml';config_xml.write(config,{'animations':False,'startup_animation':False,'restore_layout':False,'osc52_read':False,'osc52_write':False,'font_size':14.67,'theme':'dark'})
        sock=work/'server.sock'; children=[]; owner=None
        def launch(args,pipe=False):
            p=subprocess.Popen([str(x) for x in args],env=env,cwd=work,stdin=subprocess.PIPE if pipe else subprocess.DEVNULL,
                               stdout=subprocess.PIPE if pipe else subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            children.append(p);return p
        def call(op,args=None,v=1):
            owner.stdin.write(json.dumps({'v':v,'op':op,'args':args or {}}).encode()+b'\n');owner.stdin.flush()
            require(bool(select.select([owner.stdout],[],[],10)[0]),'owner reply deadline')
            value=json.loads(owner.stdout.readline());require(value.get('ok') is True,'owner RPC '+op);return value['result']
        def xdo(*args):
            p=subprocess.run(['xdotool',*map(str,args)],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
            require(p.returncode==0,f'xdotool {args[0]} exit {p.returncode}: {p.stderr.decode(errors="replace").strip()}');return p.stdout.decode().strip()
        try:
            server=launch([gpui,'--server','--socket',sock]);until(lambda:sock.exists() or (server.poll() is not None and require(False,'server startup')),'server startup')
            owner=launch([helper,'owner',sock],True)
            tab=call('open_tab',{'rows':24,'cols':80,'allow_agent':False})['tab_id']
            call('run',{'tab_id':tab,'argv':['sh','-c','sleep 600'],'cwd':str(work)})
            window=launch([gpui,'--socket',sock,'--config',config])
            def window_id():
                require(window.poll() is None,'window remained alive')
                p=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(window.pid)],env=env,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10)
                ids=p.stdout.decode().splitlines();return ids[0] if p.returncode==0 and ids else None
            wid=until(window_id,'window identity');xdo('windowsize',wid,1240,760);xdo('windowfocus','--sync',wid);time.sleep(1.2)
            def click(x,y):
                xdo('mousemove','--window',wid,x,y);xdo('click',1)
            def workspace():return call('workspace_get',{},3)
            def pane_count(tree):
                if 'Leaf' in tree:return 1
                split=tree['Split'];return pane_count(split['first'])+pane_count(split['second'])
            def tabs():
                return sorted((c for c in record_controls(rec) if c['kind']=='tab'),key=lambda c:c['box'][0])
            def tab_centre(index):
                found=until(lambda:(t:=tabs()) and len(t)>index and t,f'tab {index + 1} in the record',5)
                x,y,w,h=found[index]['box'];return x+w//2,y+h//2
            require(len(workspace()['groups'])==1,'initial session count')
            require(call('permission_get',{'tab_id':tab},3)['allow_agent'] is False,'initial access off')
            click(*record_centre(rec,lambda c:c['kind']=='segment' and c['label']=='Agents','the tools panel\'s "Agents" segment'))
            click(*record_centre(rec,lambda c:c['kind']=='switch' and c['label'].startswith('Agent access to'),
                                 'the Agents view\'s "Agent access to ..." switch'))
            until(lambda:call('permission_get',{'tab_id':tab},3)['allow_agent'] is True,'access switch toggles permission')
            print('PASS: tools panel access switch toggles owner permission off → on')
            if os.environ.get('CLANKY_GATE_FAULT')!='click':
                click(*record_centre(rec,lambda c:c['label']=='New tab','the "New tab" button'))
            try:until(lambda:len(workspace()['groups'])==2,'new tab session count',3)
            except RuntimeError:
                raise RuntimeError(f"new tab sessions: expected 2, observed {len(workspace()['groups'])}")
            print('PASS: real New tab click: sessions 1 → 2')
            click(*tab_centre(0))
            until(lambda:workspace()['active']==0,'first session selected')
            for expected in (2,3):
                xdo('key','--window',wid,'ctrl+shift+d')
                until(lambda:pane_count(workspace()['groups'][0]['layout'])==expected,f'split creates {expected} panes')
                time.sleep(.2)
            click(*tab_centre(1))
            until(lambda:workspace()['active']==1,'second session selected')
            print('PASS: 1 → 2 → 3 panes; a click on the second tab selected the second session')
        finally:
            for p in reversed(children):stop(p)


def main():
    gpui=os.environ.get('CLANKY_BIN')
    if gpui:
        return main_gpui(gpui)
    target=Path(os.environ['CARGO_TARGET_DIR']); bins=target/'debug'
    with tempfile.TemporaryDirectory(prefix='m12-click-',dir=target) as tmp:
        work=Path(tmp)
        env={k:v for k,v in os.environ.items() if not any(x in k.upper() for x in ('KEY','TOKEN','SECRET','PASSWORD','PASSWD'))}
        for key in ('WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS'):env.pop(key,None)
        for name in ('home','config','state','runtime'):(work/name).mkdir(mode=0o700)
        env.update(HOME=str(work/'home'),XDG_CONFIG_HOME=str(work/'config'),XDG_STATE_HOME=str(work/'state'),
                   XDG_RUNTIME_DIR=str(work/'runtime'),SHELL='/bin/sh',WINIT_UNIX_BACKEND='x11')
        config=work/'config.xml';config_xml.write(config,{'animations':False,'startup_animation':False,'restore_layout':False,'osc52_read':False,'osc52_write':False,'font_size':14.67,'theme':'dark'})
        sock=work/'server.sock'; children=[]; owner=None
        def launch(args,pipe=False):
            p=subprocess.Popen([str(x) for x in args],env=env,cwd=work,stdin=subprocess.PIPE if pipe else subprocess.DEVNULL,
                               stdout=subprocess.PIPE if pipe else subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            children.append(p);return p
        def call(op,args=None,v=1):
            owner.stdin.write(json.dumps({'v':v,'op':op,'args':args or {}}).encode()+b'\n');owner.stdin.flush()
            require(bool(select.select([owner.stdout],[],[],10)[0]),'owner reply deadline')
            value=json.loads(owner.stdout.readline());require(value.get('ok') is True,'owner RPC '+op);return value['result']
        def xdo(*args):
            p=subprocess.run(['xdotool',*map(str,args)],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
            require(p.returncode==0,f'xdotool {args[0]} exit {p.returncode}: {p.stderr.decode(errors="replace").strip()}');return p.stdout.decode().strip()
        try:
            server=launch([bins/'clanky','--server','--socket',sock]);until(lambda:sock.exists() or (server.poll() is not None and require(False,'server startup')),'server startup')
            owner=launch([bins/'clanky-agent-gate','owner',sock],True)
            tab=call('open_tab',{'rows':24,'cols':80,'allow_agent':False})['tab_id']
            call('run',{'tab_id':tab,'argv':['sh','-c','sleep 600'],'cwd':str(work)})
            window=launch([bins/'clanky','--socket',sock,'--config',config])
            def window_id():
                require(window.poll() is None,'window remained alive')
                p=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(window.pid)],env=env,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10)
                ids=p.stdout.decode().splitlines();return ids[0] if p.returncode==0 and ids else None
            wid=until(window_id,'window identity');xdo('windowsize',wid,1240,760);xdo('windowfocus','--sync',wid);time.sleep(.5)
            def click(x,y):
                xdo('mousemove','--window',wid,x,y);xdo('click',1)
            def saved():return config_xml.load(config)
            def shot():
                xdo('mousemove','--window',wid,600,400);time.sleep(.4)
                g=dict(line.split('=',1) for line in xdo('getwindowgeometry','--shell',wid).splitlines() if '=' in line)
                x,y=int(g['X']),int(g['Y'])
                return ImageGrab.grab(bbox=(x,y,x+1240,y+760),xdisplay=env['DISPLAY']).convert('RGB')
            def plus():
                found=marks(shot(),40,862)  # left of the 318 px search box: 1240 - 10 - 34 - 318 - 16
                require(found,'title bar marks right of the toggle');return found[-1]
            before=call('workspace_get',{},3);require(len(before['groups'])==1,'initial session count')
            # Agent access lives in the tools panel's Agents view: footer segment, then the pane's "off" row.
            time.sleep(1.2)
            require(call('permission_get',{'tab_id':tab},3)['allow_agent'] is False,'initial access off')
            agents=word(ocr_words(shot(),(0,680,236,734),tmp),'agents')
            require(agents,'tools panel footer reads "Agents"')
            click(*centre(agents))
            words=ocr_words(shot(),(0,40,236,680),tmp)
            off=min((w for w in words if w[0]=='off'),key=lambda w:w[2],default=None)
            require(off and any('access' in w[0] for w in words),'Agents view shows access and an "off" row')
            click(*centre(off))
            until(lambda:call('permission_get',{'tab_id':tab},3)['allow_agent'] is True,'access row toggles permission')
            print('PASS: tools panel access row toggles owner permission off → on')
            # The title bar's "+" adds a tab; the pointer is sent through winit.
            add=plus()
            if os.environ.get('CLANKY_GATE_FAULT')!='click':
                click((add[0]+add[1])//2,19)
            try:after=until(lambda:(w if len((w:=call('workspace_get',{},3))['groups'])==2 else None),'new tab session count',3)
            except RuntimeError:
                after=call('workspace_get',{},3)
                raise RuntimeError(f"new tab sessions: expected 2, observed {len(after['groups'])}")
            print(f"PASS: real New tab click: sessions 1 → {len(after['groups'])}")
            def workspace(): return call('workspace_get',{},3)
            def pane_count(tree):
                if 'Leaf' in tree:return 1
                split=tree['Split'];return pane_count(split['first'])+pane_count(split['second'])
            click(62,19)  # the first tab's state dot
            until(lambda:workspace()['active']==0,'first session selected')
            for expected in (2,3):
                click(1070,19);time.sleep(.4)  # the search box opens the palette
                xdo('type','--window',wid,'--delay','20','Split right');xdo('key','--window',wid,'Return')
                until(lambda:pane_count(workspace()['groups'][0]['layout'])==expected,
                      f'split creates {expected} panes')
                time.sleep(.2)
            add=plus();second=[m for m in marks(shot(),40,add[0]-3)][-1]
            click((second[0]+second[1])//2,19)  # the last mark before "+": the second tab's title
            until(lambda:workspace()['active']==1,'second session selected')
            print('PASS: real Split clicks: 1 → 2 → 3 panes; the title bar selected the second session')
            # Settings opens from the gear. Navigation and controls are the fixed 780×600 CSS geometry
            # centred in the 1240×760 fixture; the Layout segments are found by their words.
            click(1217,19);time.sleep(.4)
            click(270,188);time.sleep(.4)  # Appearance
            words=ocr_words(shot(),(459,130,985,630),tmp)
            vertical,horizontal=word(words,'vertical',max),word(words,'horizontal',max)
            require(vertical and horizontal,f'Appearance shows the Layout segments ({[w[0] for w in words][:30]})')
            click(*centre(vertical))
            until(lambda:saved().get('tab_style')=='vertical','Settings Vertical segment')
            click(*centre(horizontal))
            until(lambda:saved().get('tab_style','horizontal')=='horizontal','Settings Horizontal segment')
            body=(459,130,985,630)  # the Settings page body, right of the nav
            def row(label,notches=40):
                # Scrolls the page from its top until the label's words lie on one line in the body's middle part.
                xdo('mousemove','--window',wid,720,380);xdo('click','--repeat',30,'--delay',30,4);time.sleep(.4)
                first,second=label.lower().split() if ' ' in label else (label.lower(),None)
                for _ in range(notches):
                    words=ocr_words(shot(),body,tmp)
                    for w in words:
                        if w[0]==first and 160<=w[2]<=580 and (second is None or any(
                                v[0]==second and abs(v[2]-w[2])<=6 and 0<=v[1]-w[3]<=20 for v in words)):
                            return w,words
                    xdo('mousemove','--window',wid,720,380);xdo('click',5);time.sleep(.3)
                raise RuntimeError(f'the row "{label}" found on the page')
            size,words=row('Font size')
            value=next((w for w in words if w[0]=='14.67' and abs(w[2]-size[2])<=24 and w[1]>size[3]+40),None)
            require(value,f'the Font size field shows 14.67 ({[w[0] for w in words if abs(w[2]-size[2])<=24]})')
            click(*centre(value));time.sleep(.2)
            xdo('key','--window',wid,'ctrl+a');xdo('type','--window',wid,'--delay','25','16');xdo('key','--window',wid,'Return')
            until(lambda:saved().get('font_size')==16,'Settings font size field 14.67 -> 16')
            click(270,154);time.sleep(.4)  # General
            anim,_=row('Animations')
            click(*centre(anim))  # the Animations row's label, false → true
            until(lambda:saved().get('animations') is True,'Settings animations row')
            click(980,109);time.sleep(.2)  # close button
            old=len(workspace()['groups']);add=plus();click((add[0]+add[1])//2,19)
            until(lambda:len(workspace()['groups'])==old+1,'close button returns pointer to window')
            click(1217,19);time.sleep(.6);xdo('key','--window',wid,'Escape');time.sleep(.4)
            old=len(workspace()['groups']);add=plus();click((add[0]+add[1])//2,19)
            until(lambda:len(workspace()['groups'])==old+1,'Escape returns pointer to window')
            print('PASS: Settings section, Layout segments, font size 14.67→16, animations off→on, close and Escape')
            # A forced Post-AI switch is rendered disabled. Confirm the panel
            # really opened, then verify clicking its row does not write config.
            stop(window)
            window=launch([bins/'clanky','--socket',sock,'--config',config,'--post-ai'])
            wid=until(window_id,'forced window identity');xdo('windowsize',wid,1240,760);xdo('windowfocus','--sync',wid);time.sleep(.7)
            geometry=dict(line.split('=',1) for line in xdo('getwindowgeometry','--shell',wid).splitlines() if '=' in line)
            left,top=int(geometry['X']),int(geometry['Y'])
            def capture():return ImageGrab.grab(bbox=(left+230,top+80,left+1010,top+680),xdisplay=env['DISPLAY']).convert('RGB')
            plain=capture();click(1217,19);time.sleep(.4);panel=capture()
            changed=sum(1 for pixel in ImageChops.difference(plain,panel).getdata() if pixel!=(0,0,0))
            require(changed>5000,f'Settings open changed pixels: expected >5000, observed {changed}')
            prior=config.read_bytes();click(600,200);time.sleep(.5)
            require(config.read_bytes()==prior,'disabled Post-AI row leaves settings bytes unchanged')
            print('PASS: forced Post-AI disabled row ignores click')
        finally:
            for p in reversed(children):stop(p)


if __name__=='__main__':
    try:main()
    except (OSError,ValueError,RuntimeError,subprocess.TimeoutExpired) as error:
        print(f'FAIL: M12 click: {error}',file=sys.stderr);sys.exit(1)
