#!/usr/bin/env python3
"""Real-window hover and Copy action, observed through the owner's copy API."""
import base64
import json
import os
from pathlib import Path
import secrets
import select
import socket
import subprocess
import sys
import tempfile
import time
from PIL import ImageChops, ImageGrab


def require(ok,label):
    if not ok:raise RuntimeError(label)


def until(fn,label,seconds=12):
    end=time.monotonic()+seconds
    while time.monotonic()<end:
        value=fn()
        if value:return value
        time.sleep(.03)
    raise RuntimeError(label)


def stop(p):
    if p.poll() is None:p.terminate()
    try:p.wait(timeout=5)
    except subprocess.TimeoutExpired:p.kill();p.wait(timeout=5)


def main():
    target=Path(os.environ['CARGO_TARGET_DIR']);bins=target/'debug'
    with tempfile.TemporaryDirectory(prefix='m12-copy-',dir=target) as tmp:
        work=Path(tmp); env={k:v for k,v in os.environ.items() if not any(x in k.upper() for x in ('KEY','TOKEN','SECRET','PASSWORD','PASSWD'))}
        for key in ('WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS'):env.pop(key,None)
        for name in ('home','config','state','runtime'):(work/name).mkdir(mode=0o700)
        secret=''.join(secrets.choice('abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ') for _ in range(24));expected=b'*'*len(secret)
        env.update(HOME=str(work/'home'),XDG_CONFIG_HOME=str(work/'config'),XDG_STATE_HOME=str(work/'state'),
                   XDG_RUNTIME_DIR=str(work/'runtime'),SHELL='/bin/sh',WINIT_UNIX_BACKEND='x11',CLANKY_TEST_SECRET=secret)
        config=work/'config.json';config.write_text(json.dumps({'animations':False,'startup_animation':False,'restore_layout':False,'osc52_read':False,'osc52_write':False,'font_size':13,'theme':'dark'}))
        fixture=work/'block.py';fixture.write_text("import os,time\ns=os.environ['CLANKY_TEST_SECRET'].encode()\nos.write(1,b'\\x1b]133;A\\x07$ \\x1b]133;B\\x07echo fixture\\n\\x1b]133;C\\x07'+s+b'\\n\\x1b]133;D;0\\x07')\nwhile True: time.sleep(1)\n")
        sock=work/'server.sock';children=[];owner=None
        def launch(args,pipe=False):
            p=subprocess.Popen([str(x) for x in args],env=env,cwd=work,stdin=subprocess.PIPE if pipe else subprocess.DEVNULL,
                               stdout=subprocess.PIPE if pipe else subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            children.append(p);return p
        def call(op,args=None,v=1):
            owner.stdin.write(json.dumps({'v':v,'op':op,'args':args or {}}).encode()+b'\n');owner.stdin.flush()
            require(bool(select.select([owner.stdout],[],[],10)[0]),'owner reply deadline')
            reply=json.loads(owner.stdout.readline());require(reply.get('ok') is True,'owner RPC '+op);return reply['result']
        def xdo(*args):
            p=subprocess.run(['xdotool',*map(str,args)],env=env,stdout=subprocess.PIPE,stderr=subprocess.PIPE,timeout=10)
            require(p.returncode==0,f'xdotool {args[0]} exit {p.returncode}: {p.stderr.decode(errors="replace").strip()}');return p.stdout.decode().strip()
        try:
            server=launch([bins/'clanky','--server','--socket',sock]);until(lambda:sock.exists() or (server.poll() is not None and require(False,'server startup')),'server startup')
            owner=launch([bins/'clanky-agent-gate','owner',sock],True)
            a=call('open_tab',{'rows':13,'cols':81,'allow_agent':False})['tab_id']
            call('run',{'tab_id':a,'argv':['sh','-c','sleep 600'],'cwd':str(work)})
            b=call('open_tab',{'rows':13,'cols':81,'allow_agent':False})['tab_id']
            call('run',{'tab_id':b,'argv':[sys.executable,str(fixture)],'cwd':str(work)})
            def blocks():return call('list_blocks',{'tab_id':b,'limit':64})['blocks']
            block=until(lambda:(items[0] if (items:=blocks()) and items[0]['complete'] else None),'completed block')
            output=block['output'].encode()
            require(output==expected,f'masked block output: expected {len(expected)} stars, observed {len(output)} bytes ({output.count(b"*")} stars)')
            ws=call('workspace_get',{},3);ws['groups']=[{'layout':{'Split':{'axis':'Rows','first':{'Leaf':a},'second':{'Leaf':b}}},'focus':b}];ws['active']=0
            call('workspace_set',ws,3)
            window=launch([bins/'clanky','--socket',sock,'--config',config])
            def window_id():
                require(window.poll() is None,'window remained alive')
                p=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(window.pid)],env=env,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10)
                ids=p.stdout.decode().splitlines();return ids[0] if p.returncode==0 and ids else None
            wid=until(window_id,'window identity');xdo('windowsize',wid,1240,760);xdo('windowmove',wid,0,0);xdo('windowfocus','--sync',wid);time.sleep(1.5)
            duration=int(block.get('duration_ms') or 0)
            hit=subprocess.run([bins/'clanky-copy-hit',str(duration)],capture_output=True,text=True,timeout=20)
            require(hit.returncode==0,'scene Copy hit missing')
            hit=json.loads(hit.stdout);x,y,w,h=hit['rect'];require(w>0 and h>0,'nonempty Copy hit')
            geo=dict(line.split('=',1) for line in xdo('getwindowgeometry','--shell',wid).splitlines() if '=' in line);left,top=int(geo['X']),int(geo['Y'])
            box=(left+int(x),top+int(y),left+int(x+w),top+int(y+h))
            xdo('mousemove','--window',wid,100,50);time.sleep(.3)
            before=ImageGrab.grab(bbox=box,xdisplay=env['DISPLAY']).convert('RGB')
            xdo('mousemove','--window',wid,*hit['hover']);time.sleep(.5)
            after=ImageGrab.grab(bbox=box,xdisplay=env['DISPLAY']).convert('RGB')
            changed=sum(1 for pixel in ImageChops.difference(before,after).getdata() if pixel!=(0,0,0))
            require(changed>10,f'hover Copy pixels: expected >10 changed, observed {changed}')
            view=call('view',{'tab_id':b,'limit':64},2);require(view.get('selection') is None,'initial block selection empty')
            xdo('mousemove','--window',wid,int(x+w/2),int(y+h/2));xdo('click',1)
            def selected():
                result=call('view',{'tab_id':b,'limit':64},2).get('selection')
                return result if isinstance(result,dict) and result.get('block_id')==block['block_id'] else None
            selection=until(selected,'Copy click selects block')
            require(selection.get('start')==0 and selection.get('end')==len(expected),'Copy selects exact masked output')
            result=call('copy',{'tab_id':b,'block_id':block['block_id']},2)
            require(base64.b64decode(result['data'],validate=True)==expected,'owner copy API masked bytes')
            print(f'PASS: hover revealed Copy; real click selected and owner copy API returned {len(expected)} stars')
        finally:
            for child in reversed(children):stop(child)


if __name__=='__main__':
    try:main()
    except (OSError,ValueError,RuntimeError,subprocess.TimeoutExpired) as error:
        print(f'FAIL: M12 block hover/copy: {error}',file=sys.stderr);sys.exit(1)
