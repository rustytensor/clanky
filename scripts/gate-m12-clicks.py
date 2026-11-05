#!/usr/bin/env python3
"""First M12 click check: private real window, actual pointer, server workspace."""
import json
import os
from pathlib import Path
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


def stop(p):
    if p.poll() is None:p.terminate()
    try:p.wait(timeout=5)
    except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)


def main():
    target=Path(os.environ['CARGO_TARGET_DIR']); bins=target/'debug'
    with tempfile.TemporaryDirectory(prefix='m12-click-',dir=target) as tmp:
        work=Path(tmp)
        env={k:v for k,v in os.environ.items() if not any(x in k.upper() for x in ('KEY','TOKEN','SECRET','PASSWORD','PASSWD'))}
        for key in ('WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS'):env.pop(key,None)
        for name in ('home','config','state','runtime'):(work/name).mkdir(mode=0o700)
        env.update(HOME=str(work/'home'),XDG_CONFIG_HOME=str(work/'config'),XDG_STATE_HOME=str(work/'state'),
                   XDG_RUNTIME_DIR=str(work/'runtime'),SHELL='/bin/sh',WINIT_UNIX_BACKEND='x11')
        config=work/'config.json';config.write_text(json.dumps({'animations':False,'startup_animation':False,'restore_layout':False,'osc52_read':False,'osc52_write':False,'font_size':13,'theme':'dark'}))
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
            p=subprocess.run(['xdotool',*map(str,args)],env=env,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10)
            require(p.returncode==0,'private X11 input');return p.stdout.decode().strip()
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
            def saved():return json.loads(config.read_text())
            before=call('workspace_get',{},3);require(len(before['groups'])==1,'initial session count')
            # The access row begins after the activity card and 1–4 recent rows.
            # Click the label side, never the switch at the far right.
            time.sleep(1.2)
            recent=call('look_state',{},3).get('recent',[])
            kinds={'command_run','command_finished','agent_connected','pane_closed'}
            count=min(4,sum(item.get('kind') in kinds for item in recent))
            row_y=76+176+48+36*max(count,1)+12+40+9
            require(call('permission_get',{'tab_id':tab},3)['allow_agent'] is False,'initial access off')
            click(990,row_y)
            until(lambda:call('permission_get',{'tab_id':tab},3)['allow_agent'] is True,'access row label toggles permission')
            print('PASS: access row label toggles owner permission off → on')
            # Real top-bar New tab button at 1240x760; pointer is sent through winit.
            if os.environ.get('CLANKY_GATE_FAULT')!='click':
                xdo('mousemove','--window',wid,1180,36);xdo('click',1)
            try:after=until(lambda:(w if len((w:=call('workspace_get',{},3))['groups'])==2 else None),'new tab session count',3)
            except RuntimeError:
                after=call('workspace_get',{},3)
                raise RuntimeError(f"new tab sessions: expected 2, observed {len(after['groups'])}")
            print(f"PASS: real New tab click: sessions 1 → {len(after['groups'])}")
            def workspace(): return call('workspace_get',{},3)
            def pane_count(tree):
                if 'Leaf' in tree:return 1
                split=tree['Split'];return pane_count(split['first'])+pane_count(split['second'])
            click(40,120)
            until(lambda:workspace()['active']==0,'first session selected')
            for expected in (2,3):
                click(1080,36)
                until(lambda:pane_count(workspace()['groups'][0]['layout'])==expected,
                      f'split creates {expected} panes')
                time.sleep(.2)
            click(40,154)
            until(lambda:workspace()['active']==1,'second session selected')
            print('PASS: real Split clicks: 1 → 2 → 3 panes; sidebar selected second session')
            # Settings opens from the sidebar. Navigation and controls are the
            # fixed 780×600 CSS geometry centred in the 1240×760 fixture.
            click(40,696);time.sleep(.2)
            click(270,188);time.sleep(.2)  # Appearance
            click(820,179)  # Light segment
            until(lambda:saved().get('theme')=='light','Settings light segment')
            click(270,222);time.sleep(.2)  # Terminal
            click(965,288)  # Font size +, 13 → 14
            until(lambda:saved().get('font_size')==14,'Settings font stepper')
            click(600,534)  # Whole Animations row, false → true
            until(lambda:saved().get('animations') is True,'Settings animations row')
            click(980,109);time.sleep(.2)  # close button
            old=len(workspace()['groups']);click(1180,36)
            until(lambda:len(workspace()['groups'])==old+1,'close button returns pointer to window')
            click(40,696);time.sleep(.2);xdo('key','--window',wid,'Escape');time.sleep(.2)
            old=len(workspace()['groups']);click(1180,36)
            until(lambda:len(workspace()['groups'])==old+1,'Escape returns pointer to window')
            print('PASS: Settings section, light segment, font 13→14, animations off→on, close and Escape')
            # A forced Post-AI switch is rendered disabled. Confirm the panel
            # really opened, then verify clicking its row does not write config.
            stop(window)
            window=launch([bins/'clanky','--socket',sock,'--config',config,'--post-ai'])
            wid=until(window_id,'forced window identity');xdo('windowsize',wid,1240,760);xdo('windowfocus','--sync',wid);time.sleep(.7)
            geometry=dict(line.split('=',1) for line in xdo('getwindowgeometry','--shell',wid).splitlines() if '=' in line)
            left,top=int(geometry['X']),int(geometry['Y'])
            def capture():return ImageGrab.grab(bbox=(left+230,top+80,left+1010,top+680),xdisplay=env['DISPLAY']).convert('RGB')
            plain=capture();click(40,696);time.sleep(.4);panel=capture()
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
