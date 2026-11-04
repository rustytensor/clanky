# Agent socket API

Clanky uses UTF-8 JSON lines over a Unix-domain socket. By default the socket is
`$XDG_RUNTIME_DIR/clanky/server.sock`, with an existing private `/run/user/<uid>`
used if XDG_RUNTIME_DIR is absent. `clanky --headless --socket PATH` starts a
foreground server at a custom private path. Connect with a Unix stream socket,
write one JSON object and newline per request, and read one JSON line per reply.
The directory must belong to the user and be private. Existing sockets are not
blindly removed. Use a nonempty unique string `id` for each agent request.

For example, in Python, after connecting `s` to the socket:

```python
import json
s.sendall((json.dumps({"v": 1, "id": "1", "op": "hello"}) + "\n").encode())
reply = s.makefile("rb").readline()
```

GUI terminals deny agents by default. The owner can toggle focused-terminal
permission with Ctrl+Shift+A. Agents see and use only allowed tabs; an agent can
open its own tab with `allow_agent:true`. Operations can return errors including
`permission_denied`, `not_found`, `invalid_request` and `unknown_operation`.

Version 3 is for owners. Owner authentication requires both a private capability
and proven ancestry outside the server's terminals. Processes inside a terminal
remain agents even with the capability; unknown ancestry fails closed. Same-user
processes outside Clanky are not sandboxed, and SSH access may grant remote owner
access. Owner IDs after authentication must increase as decimal integers. Do not
log or share the capability. See [session security](design-m4.md).

**Agent output is not secret-masked.** Do not expose secrets to an agent that
should not read them.

## Examples

Each example is an independent connection to a fresh server; IDs and process IDs
are illustrative. `t0` means a nonexistent tab, so examples using it demonstrate
the real `not_found` response rather than pretending that a tab or block exists.
Version 3 examples other than hello are deliberately unauthenticated, showing the
owner-only boundary. Supply actual IDs and authenticate an eligible owner to use
those operations. `data` in v3 input and v2 copy payloads is base64.

### Version 1: tabs and commands

Request: `{"v":1,"id":"1","op":"hello"}` Response: `{"id":"1","ok":true,"result":{"version":1,"capabilities":["tabs","commands","blocks"]}}`
Request: `{"v":1,"id":"1","op":"open_tab","allow_agent":true}` Response: `{"id":"1","ok":true,"result":{"tab_id":"t1","allow_agent":true}}`
Request: `{"v":1,"id":"1","op":"list_tabs"}` Response: `{"id":"1","ok":true,"result":{"tabs":[]}}`
Request: `{"v":1,"id":"1","op":"run","tab_id":"t0","argv":["printf","hello"],"cwd":"/"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"wait","tab_id":"t0","command_id":"c1","timeout_ms":1000}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"get_exit_code","tab_id":"t0","command_id":"c1"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"get_output","tab_id":"t0","command_id":"c1","offset":0,"limit":64,"encoding":"utf8"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"list_blocks","tab_id":"t0","limit":64}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":1,"id":"1","op":"close_tab","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`

`run` requires a nonempty argv and an existing absolute cwd. Use `wait` with a
timeout up to 30,000 ms. `get_output` pages retained bytes using offset and a
limit of 1–65,536 with `utf8` or `base64` encoding. `list_blocks` uses OSC 133
markers and pages at most 64 blocks. A closed tab is not the same as a closed
client connection.

### Version 2: block actions and tasks

Request: `{"v":2,"id":"1","op":"hello"}` Response: `{"id":"1","ok":true,"result":{"version":2,"capabilities":["block_actions","tasks","palette"]}}`
Request: `{"v":2,"id":"1","op":"actions"}` Response: `{"id":"1","ok":true,"result":{"actions":["select","copy","collapse","rerun","send","task_update","view","tasks","task_get"]}}`
Request: `{"v":2,"id":"1","op":"view","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"select","tab_id":"t0","block_id":"b1","start":0,"end":3}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"copy","tab_id":"t0","block_id":"b1"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"collapse","tab_id":"t0","block_id":"b1","collapsed":true}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"rerun","tab_id":"t0","block_id":"b1","argv":["echo","hello"],"cwd":"/"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"send","tab_id":"t0","block_id":"b1","label":"Review"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"task_update","tab_id":"t0","task_id":"task1","state":"done"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"tasks","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`
Request: `{"v":2,"id":"1","op":"task_get","tab_id":"t0","task_id":"task1"}` Response: `{"id":"1","ok":false,"error":{"code":"not_found","message":"unknown tab"}}`

Selections are UTF-8 byte ranges within retained block output. Copy returns bytes;
collapse changes the view, not the output. Rerun creates a new terminal from
explicit argv and cwd. Send queues a payload; it does not launch an agent.
Task states are queued, running, done, failed and cancelled.

### Version 3: owner operations

Request: `{"v":3,"id":"1","op":"hello"}` Response: `{"id":"1","ok":true,"result":{"version":3,"server_pid":1234,"capabilities":["sessions","owner"]}}`
Request: `{"v":3,"id":"1","op":"auth","token":"invalid"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication denied"}}`
Request: `{"v":3,"id":"1","op":"workspace_get"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"shell_defaults"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"workspace_set","revision":0,"groups":[],"active":0}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"snapshot","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"history","tab_id":"t0","offset":0,"limit":64}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"scroll","tab_id":"t0","delta":1}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"input","tab_id":"t0","data":"aGVsbG8="}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"resize","tab_id":"t0","rows":24,"cols":80}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"permission_get","tab_id":"t0"}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`
Request: `{"v":3,"id":"1","op":"permission_set","tab_id":"t0","allow_agent":true}` Response: `{"id":"1","ok":false,"error":{"code":"permission_denied","message":"Owner authentication required"}}`

Workspace updates use a revision; stale revisions produce a conflict. Snapshots
may use a known generation and known image keys. History is paginated. Input is
base64 and limited to 64 KiB per request. Permission changes apply per terminal.

## M5 (in progress) — Coming in 1.0

Screen-text reads, input-wait detection, sending keys as an agent, events and an
MCP server are not implemented as M5 features yet. The editor panel, compose box
and agent secret masking are also coming in 1.0; they are not available now.
