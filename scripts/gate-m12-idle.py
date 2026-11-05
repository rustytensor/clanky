#!/usr/bin/env python3
"""Private-display real-window idle and one-key echo frame check."""
import json
import os
from pathlib import Path
import select
import subprocess
import sys
import tempfile
import time


def require(ok, label):
    if not ok:
        raise RuntimeError(label)


def until(fn, label, seconds=15):
    end = time.monotonic() + seconds
    while time.monotonic() < end:
        value = fn()
        if value:
            return value
        time.sleep(.02)
    raise RuntimeError(label)


def stop(p):
    if p.poll() is None:
        p.terminate()
    try: p.wait(timeout=5)
    except subprocess.TimeoutExpired: p.kill(); p.wait(timeout=5)


def main():
    target = Path(os.environ['CARGO_TARGET_DIR'])
    bins = target / 'debug'
    with tempfile.TemporaryDirectory(prefix='m12-idle-', dir=target) as tmp:
        work = Path(tmp)
        env = {k:v for k,v in os.environ.items() if not any(x in k.upper() for x in ('KEY','TOKEN','SECRET','PASSWORD','PASSWD'))}
        for key in ('WAYLAND_DISPLAY','DBUS_SESSION_BUS_ADDRESS'): env.pop(key, None)
        for directory in ('home','config','state','runtime'): (work / directory).mkdir(mode=0o700)
        trace = work / 'frames.csv'
        env.update(HOME=str(work/'home'), XDG_CONFIG_HOME=str(work/'config'), XDG_STATE_HOME=str(work/'state'),
                   XDG_RUNTIME_DIR=str(work/'runtime'), SHELL='/bin/sh', WINIT_UNIX_BACKEND='x11', CLANKY_FRAME_TRACE=str(trace))
        config=work/'config.json'
        config.write_text(json.dumps({'animations':False,'startup_animation':False,'restore_layout':False,'osc52_read':False,'osc52_write':False}))
        reader=work/'reader.py'
        reader.write_text("import os,tty,time\ntty.setraw(0)\nos.write(1,b'READY\\r\\n')\nb=os.read(0,1)\nopen('received','wb').write(b)\nos.write(1,b'ECHO:'+b+b'\\r\\n')\nwhile not os.path.exists('release'): time.sleep(.05)\n")
        sock=work/'server.sock'; children=[]; owner=None
        def launch(args, pipe=False):
            p=subprocess.Popen([str(x) for x in args],env=env,cwd=work,stdin=subprocess.PIPE if pipe else subprocess.DEVNULL,
                               stdout=subprocess.PIPE if pipe else subprocess.DEVNULL,stderr=subprocess.DEVNULL)
            children.append(p);return p
        def call(op,args=None,v=1):
            owner.stdin.write(json.dumps({'v':v,'op':op,'args':args or {}}).encode()+b'\n');owner.stdin.flush()
            require(bool(select.select([owner.stdout],[],[],10)[0]),'owner reply deadline')
            result=json.loads(owner.stdout.readline());require(result.get('ok') is True,'owner RPC '+op);return result['result']
        def rows():
            return trace.read_text().splitlines() if trace.exists() else []
        def count(): return sum(line.startswith('frame,') for line in rows())
        def xdo(*args):
            p=subprocess.run(['xdotool',*map(str,args)],env=env,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10)
            require(p.returncode==0,'private X11 input');return p.stdout.decode().strip()
        try:
            server=launch([bins/'clanky','--server','--socket',sock]);until(lambda:sock.exists() or (server.poll() is not None and require(False,'server startup')),'server startup')
            owner=launch([bins/'clanky-agent-gate','owner',sock],True)
            tab=call('open_tab',{'rows':24,'cols':80,'allow_agent':False})['tab_id']
            call('run',{'tab_id':tab,'argv':[sys.executable,str(reader)],'cwd':str(work)})
            until(lambda:'READY' in ''.join(call('screen',{'tab_id':tab},4)['lines']),'shell ready')
            window=launch([bins/'clanky','--socket',sock,'--config',config])
            until(lambda:count()>0 or (window.poll() is not None and require(False,'window startup')),'first frame')
            def window_id():
                require(window.poll() is None, 'window remained alive')
                result=subprocess.run(['xdotool','search','--onlyvisible','--pid',str(window.pid)],env=env,
                                      stdout=subprocess.PIPE,stderr=subprocess.DEVNULL,timeout=10)
                ids=result.stdout.decode().splitlines()
                return ids[0] if result.returncode == 0 and ids else None
            wid=until(window_id,'window identity')
            xdo('windowfocus','--sync',wid);time.sleep(.5)
            before=count();time.sleep(2);idle=count()-before
            require(idle==0,f'idle frames: expected 0, observed {idle}')
            xdo('key','--window',wid,'x')
            try:
                until(lambda:(work/'received').exists() and (work/'received').read_bytes()==b'x' and
                      'ECHO:x' in ''.join(call('screen',{'tab_id':tab},4)['lines']),'key echo in PTY')
            except RuntimeError as error:
                keys=sum(line.startswith('key,') for line in rows())
                raise RuntimeError(f'{error}; traced key events={keys}, new frames={count()-before}') from error
            until(lambda:count()>before,'key frame');time.sleep(.5);frames=count()-before
            require(1<=frames<=2,f'echo frames: expected 1..2, observed {frames}')
            print(f'PASS: real window idle {idle} frames / 2 s; key echo {frames} frame(s)')
            call('close_tab',{'tab_id':tab})
        finally:
            for child in reversed(children): stop(child)


if __name__ == '__main__':
    try: main()
    except (OSError,ValueError,RuntimeError,subprocess.TimeoutExpired) as error:
        print(f'FAIL: M12 real-window idle: {error}',file=sys.stderr);sys.exit(1)
