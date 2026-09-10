import subprocess, json, sys, time, urllib.parse, urllib.request
def get(url):
    req = urllib.request.Request(url, headers={"User-Agent":"Mozilla/5.0"})
    import json as j
    with urllib.request.urlopen(req, timeout=40) as r: return j.loads(r.read().decode("utf-8","replace"))
cursor="*"; n=0; seen=set()
with open("reviews_bl2_negative.jsonl","w",encoding="utf-8") as f:
    for i in range(12):
        url=("https://store.steampowered.com/appreviews/49520?json=1&filter=all&language=english"
             "&review_type=negative&purchase_type=all&num_per_page=100&cursor=%s"%urllib.parse.quote(cursor,safe=""))
        try: d=get(url)
        except Exception as e: print("ERR",e); break
        revs=d.get("reviews",[])
        if not revs: break
        for r in revs:
            if r["recommendationid"] in seen: continue
            seen.add(r["recommendationid"])
            f.write(json.dumps({"id":r["recommendationid"],"votes":r["votes_up"],"hours":r["author"].get("playtime_forever",0),"ts":r["timestamp_created"],"text":r["review"]})+"\n"); n+=1
        cursor=d.get("cursor","")
        if not cursor: break
        time.sleep(0.6)
print("WROTE",n)
