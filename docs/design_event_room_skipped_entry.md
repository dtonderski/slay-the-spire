# EventRoom map travel before the event screen

`EventRoom` assigns the `?` map node, then `onPlayerEntry` rolls the room
type and `generateEvent` (removing that event from the act list). The
event screen opens after `waitTimer`. SuperFastMode can publish a
command-ready MAP at that node (FIDL01297 step 275: current `?` x=3 y=1,
next `x=2` / `x=3`) before the event is choosable.

The following map CHOOSE is a child-node pick, so the rolled event is
never played. Floor 19 still consumes Beggar; floor 21 is Mausoleum.

When a map `ChooseNode` into a `?` would enter `RunPhase::Event` but the
observed subset is that MAP publication, replay keeps the post-entry RNG
and seen-list and drops the unopened event screen. Ordinary `CHOOSE` →
EVENT traces still enter because their observed subset is the event.
