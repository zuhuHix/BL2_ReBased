import json,re,collections,sys
topics=json.load(open(sys.argv[1],encoding="utf-8"))
THEMES={
 "multiplayer/coop/connection":r"co-?op|multiplayer|online|connect|join|host|lobby|lan|session|crossplay|matchmak|disconnect|нет онлайна",
 "crashes/technical/perf":r"crash|freez|fps|stutter|lag|launch|start(ing)? (issue|problem)|black screen|error|fix|bug|broken|won'?t (run|open)|performance",
 "shift/account/keys/tos":r"shift|golden key|code|account|2k|epic|tos|terms of service|privacy",
 "mods/modding":r"\bmod|blcmm|community patch|ucp|reborn|exodus|hex|texture",
 "build/gear/farming":r"build|skill|class mod|legendar|farm|drop|gear|weapon|gun|loot|pearl|seraph",
 "difficulty/leveling/uvhm":r"uvhm|op ?\d|level|xp|hard|difficult|scal|playthrough|tvhm",
 "dlc/content/story":r"dlc|season pass|headhunter|story|quest|mission|tiny tina|dragon keep|torgue",
 "graphics/settings/display":r"graphic|resolution|fov|ultrawide|texture|hd |settings|monitor|screen|4k|widescreen|vsync",
 "remaster/sequel/franchise":r"remaster|remake|bl3|bl4|borderlands 3|borderlands 4|next|sequel",
 "controller/input":r"controller|gamepad|keybind|mouse|input|steam deck|deck",
 "saves/characters":r"save|character|profile|backup|transfer|respec",
 "buying/price/refund":r"buy|price|sale|worth|refund|purchase|cost",
}
c=collections.Counter(); rc=collections.Counter()
for t in topics:
    s=t["title"].lower()
    for k,p in THEMES.items():
        if re.search(p,s):
            c[k]+=1; rc[k]+=t["replies"]
n=len(topics)
print(f"STEAM DISCUSSIONS — recent {n} threads (BL2)\n"+"="*64)
for k,_ in c.most_common():
    print(f"  {k:<32} {c[k]:>4} threads ({c[k]/n*100:>4.1f}%)  {rc[k]:>6} replies  {'#'*int(c[k]/n*60)}")
print("\n--- Most-replied recent threads ---")
for t in sorted(topics,key=lambda x:-x["replies"])[:22]:
    print(f"  {t['replies']:>6}  {t['title'][:95]}")
