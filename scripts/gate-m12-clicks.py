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
        config=work/'config.json';config.write_text(json.dumps({'animations':False,'startup_animation':False,'restore_layout':False,'osc52_read':False,'osc52_write':False}))
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
            before=call('workspace_get',{},3);require(len(before['groups'])==1,'initial session count')
            # Real top-bar New tab button at 1240x760; pointer is sent through winit.
            if os.environ.get('CLANKY_GATE_FAULT')!='click':
                xdo('mousemove','--window',wid,1180,36);xdo('click',1)
            try:after=until(lambda:(w if len((w:=call('workspace_get',{},3))['groups'])==2 else None),'new tab session count',3)
            except RuntimeError:
                after=call('workspace_get',{},3)
                raise RuntimeError(f"new tab sessions: expected 2, observed {len(after['groups'])}")
            print(f"PASS: real New tab click: sessions 1 → {len(after['groups'])}")
            def click(x,y):
                xdo('mousemove','--window',wid,x,y);xdo('click',1)
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
        finally:
            for p in reversed(children):stop(p)


if __name__=='__main__':
    try:main()
    except (OSError,ValueError,RuntimeError,subprocess.TimeoutExpired) as error:
        print(f'FAIL: M12 click: {error}',file=sys.stderr);sys.exit(1)
