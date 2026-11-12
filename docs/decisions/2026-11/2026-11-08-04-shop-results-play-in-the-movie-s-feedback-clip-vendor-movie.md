## 2026-11-08: Shop results play in the movie's feedback clip (vendor movie PR B)

AI-assisted (Claude Opus 5.5). Record: `docs/verification/VENDORS.md` section 10. Stacked on PR A.

- **Change:** in movie mode a refusal plays the shop movie's `bad` feedback with its label, replacing the page's message line. A buy or buy-back plays `purchased` and a sale `sold`, with our own labels `PURCHASED` and `SOLD`, once the host's next state of the same opening shows the money moved the right way. The HTML page is unchanged.
- **Host fields:** `OpenWillowVendor.cpp` adds a few lines. The page numbers each request (`request`). The state gains `handled`, the number of the last request answered, and `refusal`, the last action's refusal without the stock notice that `message` also carries. A result plays only for the request it answers, and `bad` only for a refusal. The page's numbers start at a random base per load. The host resets `handled` on each opening and takes only a whole JSON number from 0 to 2^53 (automation test `OpenWillow.Vendor.RequestNumber`). In movie mode a press is ignored while a request waits; the wait is let go after 3 s, and its late answer is ignored.
- **UNVERIFIED:** the real clip's timing, the words it shows for results and non-stock messages, and its sound (not played).
- **Checked:** synthetic tests (`tests/vending_movie_test.js`, and `tests/vending_test.js` running the real page code with a stub movie), and the page in headless Edge. The in-engine suite has not run it.

