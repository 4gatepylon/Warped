# How to check each feature

One row per feature of the prototype: what to run or open, and what you should see. Start the prototype with
`make -C demos/system_design_visualization_and_prototyping run-prototype` and open http://127.0.0.1:8010/.
`make test` runs every automated check; `make capture-flows` plays the flows marked "flow" in Chrome, checks each
screen and writes screenshots to `.artifacts/capture_flows/<run_id>/`.

| Feature | Run or open | You should see |
| --- | --- | --- |
| Everything automated | `make test` | every test passes (76 at part 2) |
| Fleet and caps (part 1) | `/system/`, set players online to 1000, press +10 MIN | machines boot, game servers fill, at most 100 matches; the rest wait in the wait queue |
| Settings live (part 1) | `/site/` → Superadmin, change a setting and save | the value is kept, and the simulation uses it from the next tick |
| Status page | `/site/` → Status | "Sign-in with Google up" and "Payments up" |
| Sign-up (flow `sign_up`) | `/game/` → Sign in with Google → Ada Lovelace | Google's stand-in page, then the first-time form; "warped_admin" is refused as imitating staff and a 2015 birth date as under 13; a good name shows "free"; Create account opens Home with the name, the icon and "Free online play left: 1 finished match" |
| Purchase (flow `purchase`) | sign in as someone new, sim band +1 FINISHED MATCH, Home → PLAY → PAY IN THE BROWSER | Home says the free allowance is used up; PLAY opens Purchase; Stripe's stand-in page opens in a new tab; 4000 0000 0000 0002 is declined; 4242 4242 4242 4242 lands on "Thank you"; the app shows "Online play: bought." within a second |
| Webhook sent twice | after paying, sim band SEND LAST WEBHOOK AGAIN | nothing changes; the system view's messages show the second delivery |
| Refund | sim band REFUND | Home says the free allowance is used up again, and PLAY offers the purchase |
| One session per account (flow `sessions`) | sign in to one account in two tabs | the first tab goes to Sign-in with "signed out because your account signed in on another computer" |
| Name change | Settings → Account, change the name twice | the first works; the second says the date it is allowed again, 30 days later |
| Delete account | Settings → Account → DELETE ACCOUNT → DELETE FOREVER | back at Sign-in, saying the account was deleted; the system view's `account` table shows the placeholder name |
| EU refused (flow `eu`) | sim band "Connects from" France, then sign up | "Warped isn't offered in the European Union." |
| Secrets hidden | `/system/` → Tables, "Kept by: Secret Manager" | every value reads "(hidden)" |
| Components visible | `/system/` after any of the above | Game app, Google (fake), Stripe (fake), Backend, Database and Secret Manager, with numbered arrows for the messages between them |
