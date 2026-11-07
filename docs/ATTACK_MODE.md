# Attack mode and the action queue

Options > Feedback has a row **Auto attack mode: On** (default). Code: [stance.rs](../crates/kotor-game/src/stance.rs).

## The button tells the truth

Each party member has their own attack mode. The Attack mode button shows the leader's, and each companion's portrait gets the same
yellow line when theirs is on. A member counts as in attack mode when it is on **or** they are fighting, so the line never lies.

- A click on a dark button turns attack mode on. A click on a lit one turns it off, which **ends the fight**: the target is dropped,
  the chase stops, what they were told to do next and their queue are dropped. This is what the Disengage button did, and
  **the Disengage button is gone** (its line at the top now says "Press the Attack mode button to cancel").
- The Cancel Combat key (F in a fight) does the same as that click.
- Hold Ctrl and click a companion's portrait to turn their mode, without changing who leads.
- After you turn the mode off in a fight, it stays off until the fight is over: it does not light again while enemies hit you, and a
  companion you turned off holds fire (does not join your target). One that is attacked still defends itself.

## Auto attack mode

With the switch on, a member who enters a fight (they attack an enemy, or an enemy attacks them) goes into attack mode by
themselves, the leader and each companion alike. A mode that came on by itself goes off again 4 seconds after their fight ends
(`AUTO_OFF_SECONDS`). A mode you set by hand stays until you turn it off. Off: nothing goes on by itself, and the button still
lights while they fight. A scripted pilot (the drivers) keeps the old rules.

## The queue

Each member has their own queue, kept when the lead changes. One thing happens each round, in order.

| How | What |
|---|---|
| Click a slot under an enemy's bar | queues its feat or power and starts the attack |
| Shift and click | queues only: the plain attack, a feat, a power or a throwable; nothing starts |
| The circle in the queue strip | takes back the last thing queued |
| The strip | clears the queue |

- Holds up to **3**: the strip has four places, and the first is the action under way.
- It works while paused. Resuming plays the queue from the front.
- A power or item that cannot be used (no Force, out of reach, the item gone) is dropped and the next one is tried in the same round.
- The strip shows the pictures in the order they will happen.

## Assumed

- "Fighting" means attacking an enemy or being attacked by one.
- 4 seconds is "a few".
- The queue's size, and the circle and strip as undo and clear, are mine: the game's own Clear buttons are the Disengage text.
