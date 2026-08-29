# Combat-generated reward hand order

The three in-combat reward surfaces do not share one visible hand insertion rule.
CommunicationMod traces are authoritative for the stable state after each choice:

- Toolbox inserts its chosen opening card at the front of the hand.
- Discovery inserts its chosen card at the front of the hand.
- Colorless Potion appends its chosen card after the cards already in hand.

Keep these branches explicit rather than normalizing them through one helper. The
live `session-1203.jsonl` transition at step 7600 proves the Colorless Potion case:
the existing `Power Through`, `Strike_R` hand is followed by `Sadistic Nature`.
