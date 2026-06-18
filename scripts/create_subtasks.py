#!/usr/bin/env python3
"""Create LoreGUI per-operation subtasks under their domain Stories (SBAI).

Idempotency: skips creating a subtask whose summary already exists under the parent.
Auth via env: JIRA_USER, JIRA_TOKEN. Reads nothing else.
"""
import base64, json, os, sys, time, urllib.request, urllib.error, urllib.parse

BASE = "https://biloxistudios.atlassian.net"
USER = os.environ["JIRA_USER"]
TOKEN = os.environ["JIRA_TOKEN"]
AUTH = base64.b64encode(f"{USER}:{TOKEN}".encode()).decode()
HDR = {"Authorization": f"Basic {AUTH}", "Content-Type": "application/json", "Accept": "application/json"}
PROJECT = "SBAI"

# domain -> (story_key, [ops])
PLAN = {
    "auth": ("SBAI-3686", "login_interactive login_with_token resolve_user_info local_user_info list logout clear".split()),
    "repository": ("SBAI-3687", "clone info dump create create_with_metadata delete release flush gc list status verify_state verify_fragment store_immutable_query metadata_get metadata_set metadata_clear instance_list instance_prune repository_update_path config_get".split()),
    "branch": ("SBAI-3688", "create info diff list latest_list switch push reset archive protect unprotect merge_start merge_into merge_resolve merge_resolve_mine merge_resolve_theirs merge_unresolve merge_restart merge_abort metadata_get metadata_set metadata_clear".split()),
    "revision": ("SBAI-3689", "commit commit_with_metadata amend info history diff find find_local sync restore bisect metadata_get metadata_set metadata_list metadata_clear cherry_pick cherry_pick_local cherry_pick_abort cherry_pick_unresolve cherry_pick_restart cherry_pick_resolve cherry_pick_resolve_mine cherry_pick_resolve_theirs revert revert_local revert_abort revert_unresolve revert_restart revert_resolve revert_resolve_mine revert_resolve_theirs".split()),
    "file": ("SBAI-3690", "stage stage_move stage_merge unstage dirty dirty_move dirty_copy reset reset_to_last_merged obliterate info history diff write hash dump metadata_get metadata_set metadata_list metadata_clear".split()),
    "lock": ("SBAI-3691", "file_acquire file_acquire_as_owner file_status file_query file_release".split()),
    "link": ("SBAI-3692", "add remove update list list_staged".split()),
    "layer": ("SBAI-3693", "layer_add layer_remove layer_list layer_list_staged".split()),
    "storage": ("SBAI-3694", "open close flush put put_file get get_file get_metadata copy obliterate upload".split()),
    "shared_store": ("SBAI-3695", "create info set_use_automatically".split()),
    "service": ("SBAI-3696", "start stop".split()),
    "notification": ("SBAI-3697", "subscribe unsubscribe".split()),
    "dependency": ("SBAI-3698", "dependency_add dependency_remove dependency_list".split()),
}


def api(method, path, body=None):
    data = json.dumps(body).encode() if body is not None else None
    req = urllib.request.Request(BASE + path, data=data, headers=HDR, method=method)
    try:
        with urllib.request.urlopen(req) as r:
            return json.loads(r.read().decode() or "{}")
    except urllib.error.HTTPError as e:
        sys.stderr.write(f"HTTP {e.code} {method} {path}: {e.read().decode()[:400]}\n")
        raise


SUBTASK_TYPE_ID = "10061"  # SBAI "Subtask" (discovered via createmeta/SBAI/issuetypes)


def existing_children(parent):
    jql = f'parent = {parent}'
    r = api("GET", f"/rest/api/3/search/jql?jql={urllib.parse.quote(jql)}&fields=summary&maxResults=200")
    return {i["fields"]["summary"] for i in r.get("issues", [])}


def adf(text):
    return {"type": "doc", "version": 1, "content": [
        {"type": "paragraph", "content": [{"type": "text", "text": text}]}]}


def main():
    st_id = SUBTASK_TYPE_ID
    sys.stderr.write(f"subtask type id: {st_id}\n")
    out = open("/srv/studiobrain-dev/loregui/docs/jira-subtasks.tsv", "w")
    out.write("domain\top\tkey\tparent\n")
    total = 0
    for domain, (story, ops) in PLAN.items():
        have = existing_children(story)
        for op in ops:
            summary = f"LoreGUI [{domain}] op: {op}"
            if summary in have:
                sys.stderr.write(f"skip exists: {summary}\n")
                continue
            desc = (f"Implement lore-vm::ops::{domain}::{op} (LoreApi facade) + a #[tauri::command] wrapper "
                    f"+ a GUI affordance (panel action or command-palette entry) + an integration test against a "
                    f"temp repo+shared-store. ONE file per layer; do NOT edit shared registries (manager merges them). "
                    f"Bind the upstream `lore` crate fn lore::{domain}::{op} (no CLI shelling). "
                    f"Repo: BiloxiStudios/loregui. Branch: SBAINNNN-{domain}-{op}. PR title: 'SBAI-NNNN: {domain} {op}'. "
                    f"BLOCKED BY SBAI-3685 (Foundation infra) — do not start until Foundation merges. "
                    f"See docs/IMPLEMENTATION-PLAN.md §4 for the exact binding pattern.")
            body = {"fields": {
                "project": {"key": PROJECT},
                "parent": {"key": story},
                "issuetype": {"id": st_id},
                "summary": summary,
                "description": adf(desc),
                "labels": ["loregui", "repo-loregui", f"loregui-{domain.replace('_','-')}", "blocked-foundation"],
            }}
            r = api("POST", "/rest/api/3/issue", body)
            out.write(f"{domain}\t{op}\t{r['key']}\t{story}\n")
            total += 1
            sys.stderr.write(f"created {r['key']}: {summary}\n")
            time.sleep(0.15)
    out.close()
    sys.stderr.write(f"DONE: {total} subtasks created\n")


if __name__ == "__main__":
    main()
