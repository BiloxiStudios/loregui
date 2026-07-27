#!/usr/bin/env python3
import os
import subprocess
import requests
import sys

def check():
    root = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
    os.chdir(root)
    print(f'--- Lore Setup Verification (Root: {root}) ---')

    # 1. Check lorevm
    lorevm = './target/release/lorevm'
    if not os.path.isfile(lorevm):
        lorevm = './LoreGUI_Linux_x64'
    
    if os.path.isfile(lorevm):
        print('Checking lorevm... ', end='', flush=True)
        try:
            subprocess.run([lorevm, '--list'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True)
            print('✓ OK')
        except Exception as e:
            print(f'✗ FAILED ({e})')
    else:
        print(f'⚠ lorevm binary not found at {lorevm}')

    # 2. Check lore-mcp
    mcp_server = 'lore-mcp/server.py'
    mcp_venv = 'lore-mcp/venv/bin/python'
    if os.path.isfile(mcp_server) and os.path.isfile(mcp_venv):
        print('Checking lore-mcp... ', end='', flush=True)
        env = os.environ.copy()
        env['LOREVM_BIN'] = os.path.abspath(lorevm)
        try:
            subprocess.run([mcp_venv, mcp_server, '--list'], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=True, env=env)
            print('✓ OK')
        except Exception as e:
            print(f'✗ FAILED ({e})')
    else:
        print('⚠ lore-mcp or venv not found')

    # 3. Check LoreGUI
    print('Checking LoreGUI process... ', end='', flush=True)
    try:
        # Use pgrep equivalent
        if sys.platform == 'win32':
            output = subprocess.check_output(['tasklist', '/FI', 'IMAGENAME eq LoreGUI.exe'], stderr=subprocess.DEVNULL)
            running = b'LoreGUI.exe' in output
        else:
            output = subprocess.check_output(['pgrep', '-x', 'LoreGUI'], stderr=subprocess.DEVNULL)
            running = len(output) > 0
        if running:
            print('✓ RUNNING')
        else:
            # try lowercase
            output = subprocess.check_output(['pgrep', '-x', 'loregui'], stderr=subprocess.DEVNULL)
            if len(output) > 0:
                print('✓ RUNNING')
            else:
                print('✗ NOT RUNNING (optional)')
    except:
        print('✗ NOT RUNNING (optional)')

    # 4. Check loreserver
    print('Checking loreserver health... ', end='', flush=True)
    try:
        resp = requests.get('http://localhost:41339/status', timeout=2)
        if resp.status_code == 200 and resp.json().get('running'):
            print('✓ OK (reachable)')
        else:
            print(f'✗ FAILED (status {resp.status_code})')
    except Exception as e:
        print('✗ NOT REACHABLE (optional)')

    print('--- Verification Complete ---')

if __name__ == '__main__':
    check()
