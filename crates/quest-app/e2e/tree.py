"""Reads a Rune `--tree` JSON and answers questions about the task rows on Today.

usage: tree.py TREE has TITLE        exit 0 if a task row titled TITLE is shown
       tree.py TREE id TITLE         print that row's task id
       tree.py TREE done TITLE       exit 0 if that row's "Complete" checkbox is checked
       tree.py TREE shows TEXT       exit 0 if any element's text contains TEXT
       tree.py TREE key KEY          exit 0 if an element keyed KEY is shown
       tree.py TREE checked KEY      exit 0 if the element keyed KEY (a switch) is checked
       tree.py TREE keytext KEY TEXT exit 0 if the element keyed KEY shows exactly TEXT
       tree.py TREE workspace NAME   print the id of the workspace NAME in Manage workspaces
"""

import json
import re
import sys

CARD = re.compile(r"row-([0-9a-f-]{36})")


def card_id(tree, title):
    for element in tree["elements"]:
        if element.get("text") != title:
            continue
        match = CARD.search(element.get("path", ""))
        if match:
            return match.group(1)
    return None


def find_key(node, key):
    if node.get("key") == key:
        return node
    for child in node.get("children", []):
        found = find_key(child, key)
        if found:
            return found
    return None


def main():
    path, question, title = sys.argv[1:4]
    with open(path) as f:
        tree = json.load(f)
    if question == "quest":
        for element in tree["elements"]:
            match = re.search(r"nav-quest-([0-9a-f-]{36})", element.get("path", ""))
            if element.get("text") == title and match:
                print(match.group(1))
                return
        sys.exit(1)
    if question == "keytext":
        wanted = sys.argv[4]
        sys.exit(0 if any(e.get("key") == title and e.get("text") == wanted for e in tree["elements"]) else 1)
    if question == "workspace":
        prefix = "workspace-name-"
        for element in tree["elements"]:
            key = element.get("key") or ""
            if key.startswith(prefix) and element.get("text") == title:
                print(key[len(prefix):])
                return
        print(f"no workspace named {title!r}", file=sys.stderr)
        sys.exit(1)
    if question == "shows":
        sys.exit(0 if any(title in (e.get("text") or "") for e in tree["elements"]) else 1)
    if question == "key":
        sys.exit(0 if any(e.get("key") == title for e in tree["elements"]) else 1)
    if question == "below":
        rects = {e.get("key"): e.get("rect") for e in tree["elements"]}
        item, viewport = rects.get(title), rects.get(sys.argv[4])
        sys.exit(0 if item and viewport and item[1] >= viewport[1] + viewport[3] else 1)
    if question == "below":
        rects = {e.get("key"): e.get("rect") for e in tree["elements"]}
        item, viewport = rects.get(title), rects.get(sys.argv[4])
        sys.exit(0 if item and viewport and item[1] >= viewport[1] + viewport[3] else 1)
    if question == "checked":
        node = find_key(tree["accessibility"], title)
        sys.exit(0 if node and "checked" in node.get("states", []) else 1)
    task = card_id(tree, title)
    if question == "has":
        sys.exit(0 if task else 1)
    if task is None:
        print(f"no task row titled {title!r}", file=sys.stderr)
        sys.exit(1)
    if question == "id":
        print(task)
        return
    if question == "above":
        anchor = card_id(tree, sys.argv[4])
        titles = [e.get("key") for e in tree["elements"]]
        sys.exit(0 if anchor and titles.index(f"row-title-{task}") < titles.index(f"row-title-{anchor}") else 1)
    if question == "under":
        day = sys.argv[4]
        sys.exit(0 if any(e.get("key") == f"row-title-{task}" and f"plan-day-{day}" in e.get("path", "") for e in tree["elements"]) else 1)
    if question == "rowtext":
        wanted = sys.argv[4]
        sys.exit(0 if any(f"row-{task}" in e.get("path", "") and e.get("text") == wanted for e in tree["elements"]) else 1)
    if question in ("done", "open"):
        check = find_key(tree["accessibility"], f"row-check-{task}")
        sys.exit(0 if check and ("checked" in check.get("states", [])) == (question == "done") else 1)
    sys.exit(f"unknown question {question!r}")


if __name__ == "__main__":
    main()
