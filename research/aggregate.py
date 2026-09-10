import json,re,os,collections

# ---- unified theme lexicon (applied to every source) ----
T = {
 # praise
 "P: writing / humour / tone":      r"writ(ing|er)|humou?r|funny|hilarious|comedy|joke|dialogue|script",
 "P: Handsome Jack / villain":      r"handsome jack|best villain|antagonist|dameon clarke",
 "P: loot chase / guns":            r"\bloot\b|gun ?play|\bguns?\b|weapons?|legendar|pearl|farm|drop",
 "P: co-op with friends":           r"co-?op|with (my )?friends?|multiplayer|couch",
 "P: characters / skill trees":     r"vault hunter|skill ?tree|build|salvador|krieg|zer0|maya|gaige|axton|respec",
 "P: DLC quality":                  r"\bdlc\b|tiny tina|dragon keep|torgue|hammerlock|scarlett|headhunter",
 "P: art style holds up":           r"art ?style|cel[- ]?shad|timeless|holds up|still looks",
 "P: replayability / hours":        r"replay|\d{2,4} ?(hours|hrs)|hundreds of hours|still play|nostalg",
 # complaints / asks
 "C: multiplayer / connection BROKEN": r"co-?op.{0,30}(broken|doesn'?t|can'?t|bug)|multiplayer.{0,30}(broken|doesn'?t|dead|cooked)|can'?t (join|connect)|creating online session|disconnect|hardlock|looping loading|matchmak|crossplay|netcode|\bhost\b.{0,20}(lag|issue)",
 "C: SHiFT / account / ToS / ads":  r"\bshift\b|2k account|epic (games )?account|account link|spyware|privacy polic|terms of service|\btos\b|\bads?\b.{0,20}(borderlands|bl3|bl4)|denuvo|anti.?cheat",
 "C: crashes / stability / perf":   r"crash|freez|stutter|won'?t (launch|start|run)|black screen|fatal error|fps ?(drop|issue)|performance|out of (video )?memory|optimi[sz]",
 "C: 32-bit / memory / UHD pack":   r"32[- ]?bit|64[- ]?bit|\bram\b|out of video memory|ultra ?hd|hd texture|uhd",
 "C: slag requirement":             r"\bslag\b",
 "C: UVHM / OP levels / scaling":   r"\buvhm\b|ultimate vault hunter|op ?(level|\d)|bullet ?spong|health ?gate|scaling|\btvhm\b",
 "C: grind / RNG / drop rates":     r"grind|tedious|\brng\b|drop ?rate|farm.{0,15}(hour|forever|tedious)",
 "C: inventory / bank / UI":        r"inventor|bank ?(space|slot)|backpack|\bui\b\b|menus?|sort(ing)?|\bhud\b|item card",
 "C: backtracking / fast travel":   r"backtrack|fast ?travel|running back|walk back",
 "C: movement / gunplay feel dated":r"movement|slide|mantle|clunky|janky|aged|recoil|feels? (old|dated|bad|weak)|ads\b",
 "C: FOV / ultrawide / resolution": r"\bfov\b|field of view|ultra ?wide|21:9|widescreen|\b4k\b|resolution",
 "C: no PC split-screen":           r"split.?screen|couch co-?op|local co-?op|nucleus",
 "C: enemy design annoyances":      r"bullymong|varkid|badass.{0,20}annoy|loader|rakk|enemies.{0,20}annoy|spong",
 "C: level cap / endgame":          r"level ?cap|endgame|end ?game|op ?8|op ?10|lvl ?(72|80)",
 "C: needs mods to be good":        r"\bmods?\b|blcmm|community patch|\bucp\b|reborn|exodus|bl2 ?fix|hex ?edit",
 "C: wants remaster / remake":      r"remaster|remake|next.?gen|re-?release|modern graphics",
}

SOURCES = []
def add_texts(label, texts, weight=1.0):
    SOURCES.append((label, texts, weight))

def jl(p, key="text"):
    if not os.path.exists(p): return []
    return [json.loads(l)[key] for l in open(p,encoding="utf-8") if l.strip()]

add_texts("Steam reviews (recent)",   jl("reviews_bl2_recent.jsonl"))
add_texts("Steam reviews (helpful)",  jl("reviews_bl2_helpful.jsonl"))
add_texts("Steam reviews (negative)", jl("reviews_bl2_negative.jsonl"))
try:
    add_texts("Steam forum threads", [t["title"] for t in json.load(open("steam_topics_bl2.json",encoding="utf-8"))])
except Exception: pass
try:
    rd=json.load(open("reddit_ddg_all.json",encoding="utf-8"))
    add_texts("Reddit threads", [ (r["title"]+" "+r.get("snippet","")) for r in rd ])
except Exception: pass

print("SOURCE SIZES")
for lab,txts,_ in SOURCES: print(f"  {lab:<26} n={len(txts)}")

rates = {}
for lab, txts, _ in SOURCES:
    n = max(len(txts),1)
    c = collections.Counter()
    for t in txts:
        s = t.lower()
        for k,p in T.items():
            if re.search(p, s): c[k]+=1
    rates[lab] = {k: c[k]/n*100 for k in T}

labs=[l for l,_,_ in SOURCES]
print("\n\nCROSS-SOURCE THEME PREVALENCE (% of items in that source mentioning the theme)")
print("="*118)
print(f"{'THEME':<38}" + "".join(f"{l.replace('Steam ','St.').replace('reviews ','rv')[:15]:>16}" for l in labs) + f"{'MEAN':>9}")
print("-"*118)
rows=[]
for k in T:
    vals=[rates[l][k] for l in labs]
    rows.append((k, vals, sum(vals)/len(vals)))
for k,vals,mean in sorted(rows,key=lambda r:-r[2]):
    print(f"{k:<38}" + "".join(f"{v:>15.1f}%" for v in vals) + f"{mean:>8.1f}%")
