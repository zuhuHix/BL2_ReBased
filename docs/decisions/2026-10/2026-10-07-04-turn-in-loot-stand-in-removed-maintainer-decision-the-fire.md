## 2026-10-07: turn-in loot stand-in removed (maintainer decision); the Fire mission gives XP only

Maintainer decision: "remove the turn in loot since that's not how missions are gonna look." AI-assisted (Claude), lane I4. The stock data
gives `M_RockPaperGenocide_Fire` no item reward (only `ExperienceRewardPercentage` and a credit multiplier of 0), and the script's
`GetItemRewardsForPlayer` returns nothing, so the turn-in now gives XP only. Removed: `weapon_slice_gear.py --reward-only` and its seed search,
the host's reward pickup, and the `reward_roll` recipe. Kept: the pickup, backpack and use-key pickup code that the lent mission pistol and the
inventory suite use. Quest checks `turn_in_drops_loot_stand_in_pickup` and `use_key_collects_loot_pickup_into_backpack` were removed;
`turn_in_gives_no_item_reward` was added. Other machines should delete a stale local `slice_reward_roll.json`. Checks: ctest 11/11,
verify_packages 9/9, UE build succeeded, quest suite 97/97 + resume 11/11 (98 - 2 + 1), door suite 16/16, inventory suite PASS 49 FAIL 0
(same as the last run).

