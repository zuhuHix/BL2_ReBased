import json, re, sys, collections, os

def load(p):
    out=[]
    if not os.path.exists(p): return out
    for line in open(p,encoding="utf-8"):
        line=line.strip()
        if line: out.append(json.loads(line))
    return out

THEMES = {
 "handsome_jack/villain": r"handsome jack|\bjack\b.{0,20}villain|best villain|antagonist",
 "writing/humor": r"\bwriting\b|\bhumou?r\b|\bfunny\b|hilarious|comedy|\bjokes?\b|\bdialogue\b",
 "story": r"\bstory\b|\bnarrative\b|\bplot\b",
 "loot/guns": r"\bloot\b|\bguns?\b|\bweapons?\b|gun ?play|billion guns|\bfarming\b|\blegendar",
 "coop/friends": r"\bco-?op\b|\bfriends?\b|multiplayer|\bsplit-?screen\b",
 "characters/vault hunters": r"vault hunter|\bsalvador\b|\bkrieg\b|\bzer0\b|\bmaya\b|\bgaige\b|\baxton\b|\bskill tree\b|\bbuilds?\b",
 "dlc quality": r"\bdlc\b|tiny tina|assault on dragon|torgue|hammerlock",
 "art style/cel shaded": r"art style|cel[- ]?shad|graphics? (still|hold)|\bvisuals?\b|\bartstyle\b",
 "nostalgia/replay": r"nostalg|replay|\d+ ?(hours|hrs)|hundreds of hours|still play",
 "value/price": r"\bcheap\b|\bsale\b|worth (it|every)|\bvalue\b|\bprice\b",
 "--- COMPLAINTS ---": r"\x00",
 "crashes/stability": r"\bcrash|\bfreez|\bstutter|won'?t (launch|start)|black screen|\bfps\b.{0,15}(drop|issue)|performance issue",
 "shift/epic/login": r"\bshift\b|\bsheft\b|2k account|epic (games )?account|\blogin\b|\baccount link|\bspyware\b|privacy polic",
 "crossplay/matchmaking/online": r"\bcrossplay\b|matchmak|can'?t (join|connect)|\bhost\b.{0,15}(lag|issue)|\blag\b|\bnetcode\b|online (broken|issue)|multiplayer (broken|doesn'?t work)",
 "slag requirement": r"\bslag\b",
 "uvhm/scaling difficulty": r"\buvhm\b|ultimate vault hunter|op ?(levels?|8|10)|bullet ?spong|health gate|scaling",
 "grind/farming tedium": r"\bgrind|\btedious\b|\brng\b|drop rate",
 "backtracking/travel": r"backtrack|fast travel|\brunning back|\bvehicle.{0,15}(bad|clunk)",
 "ui/inventory/menus": r"\bui\b|\bhud\b|\binventory\b|\bmenus?\b|bank space|backpack space|\bsort(ing)?\b",
 "fov/ultrawide/resolution": r"\bfov\b|ultra ?wide|21:9|\bfield of view\b|\b4k\b|resolution",
 "32bit/memory/optimization": r"32[- ]?bit|64[- ]?bit|\bmemory\b|\bram\b|out of video|optimi[sz]",
 "controls/aim/console port": r"aim ?assist|\bcontroller\b|\bkeybind|mouse (accel|smooth)|console port|\bclunky\b",
 "enemies (bullymongs/varkids/etc)": r"bullymong|\bvarkid|\bloader\b|\brakk\b|badass.{0,15}annoy|\bnomad\b|enemies are annoying",
 "mods needed": r"\bmod(s|ded|ding)?\b|\bucp\b|community patch|\breborn\b|\bblcmm\b",
 "remaster/remake wanted": r"remaster|\bremake\b|\bupdate the game\b|next[- ]?gen",
 "level cap/progression": r"level cap|\bop8\b|\bop10\b|endgame|end ?game",
}

def analyze(path, label):
    rows = load(path)
    if not rows: 
        print(f"\n### {label}: NO DATA\n"); return
    n = len(rows)
    counts = collections.Counter()
    for r in rows:
        t = r["text"].lower()
        for k, pat in THEMES.items():
            if re.search(pat, t):
                counts[k]+=1
    print(f"\n{'='*70}\n### {label}  (n={n})\n{'='*70}")
    for k in THEMES:
        if k.startswith("---"):
            print("  " + k); continue
        c = counts[k]
        bar = "#"*int(c/n*100/2)
        print(f"  {k:<38} {c:>5}  {c/n*100:>5.1f}%  {bar}")

for path,label in [
  ("reviews_bl2_recent.jsonl","BL2 — RECENT reviews"),
  ("reviews_bl2_helpful.jsonl","BL2 — MOST HELPFUL (all time)"),
  ("reviews_bl2_negative.jsonl","BL2 — NEGATIVE only"),
  ("reviews_bl4_recent.jsonl","BL4 — RECENT reviews"),
  ("reviews_bl3_helpful.jsonl","BL3 — MOST HELPFUL"),
]:
    analyze(path,label)
