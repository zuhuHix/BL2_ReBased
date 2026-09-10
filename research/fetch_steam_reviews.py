import json, sys, time, urllib.parse, urllib.request

APP = sys.argv[1] if len(sys.argv) > 1 else "49520"
OUT = sys.argv[2] if len(sys.argv) > 2 else "reviews_49520.jsonl"
FILTER = sys.argv[3] if len(sys.argv) > 3 else "recent"
PAGES = int(sys.argv[4]) if len(sys.argv) > 4 else 15

def get(url):
    req = urllib.request.Request(url, headers={"User-Agent": "Mozilla/5.0"})
    with urllib.request.urlopen(req, timeout=40) as r:
        return json.loads(r.read().decode("utf-8", "replace"))

cursor = "*"
seen = set()
n = 0
with open(OUT, "w", encoding="utf-8") as f:
    for i in range(PAGES):
        url = ("https://store.steampowered.com/appreviews/%s?json=1&filter=%s&language=english"
               "&review_type=all&purchase_type=all&num_per_page=100&cursor=%s"
               % (APP, FILTER, urllib.parse.quote(cursor, safe="")))
        try:
            d = get(url)
        except Exception as e:
            print("ERR", e); break
        revs = d.get("reviews", [])
        if i == 0:
            print("SUMMARY", json.dumps(d.get("query_summary", {})))
        if not revs:
            break
        for r in revs:
            rid = r["recommendationid"]
            if rid in seen:
                continue
            seen.add(rid)
            f.write(json.dumps({
                "id": rid, "up": r["voted_up"], "votes": r["votes_up"],
                "funny": r["votes_funny"],
                "hours": r["author"].get("playtime_forever", 0),
                "ts": r["timestamp_created"], "text": r["review"]
            }) + "\n")
            n += 1
        cursor = d.get("cursor", "")
        if not cursor:
            break
        time.sleep(0.6)
print("WROTE", n, "reviews to", OUT)
