# NPC appearance and detail-window turn-in — 2026-09-23

The player showed BichonWall Board and MirGuide Peter using the same male NPC
sprite, followed by a ready level-19 cave patrol whose detail window offered
Share instead of Finish. The task card incorrectly directed the player to
Teleport Kyle although the authored return point was Board.

## Causes and repair

Native packet-first `ObjectNpc`/`NewNpcInfo` carries an `image` number without
the additional world-snapshot `sprite` object. The adapter discarded that
visual identity and the renderer selected its generic `NPC/00` fallback.
Native now derives the library from the wire image: ordinary images use
`NPC/{image:02}`, and Crystal's separate 1000–1099 range uses `Flag/00..99`.
Explicit packet sprite metadata remains authoritative; derived metadata does
not override an explicit full-snapshot sprite. A private merge marker is
removed before rendering.

The imported Board is image 45, loaded object 24, static NPC index 35, at
Bichon `(334,259)`. Peter is image 8/object 26. The original exported images
were inspected: Board is the notice board, Peter is a robed NPC. Their native
frame-0 widths are respectively 140 and 60 pixels. Kyle is object 17/static
index 24 at `(371,314)`. Matching quest endpoint 24 to the big-map static
index selected Kyle. Guidance now follows the server's loaded-object-first
canonical lookup, with static-index fallback only for legacy imports. V2
main and growth quests require loaded IDs. Ordinary q51/q52 also resolve
object 33 to Alice, rather than static index 33/Merchant Bull. This lookup
works before the large-map cache is populated.

Ready NPC quests now show Finish in their independent detail window. One
explicit click validates the current quest, reward selection, map and visible
NPC within the existing 16-tile data range. It opens that NPC through the
ordinary interaction command, waits at most five seconds for the correct
server-offered finish link, then sends the ordinary FinishQuest operation.
Already-authorized conversations use the same finish path directly. There
is no local completion, grant, teleport or server permission relaxation.
Remote clicks explain the correct return point instead of issuing a request.

The pending click is cancelled on Escape, closing/changing the detail,
reward changes, completion, map/epoch changes, lost NPC/range, timeout, death,
Notice or another blocking modal. Native dispatch revalidates the specific
pending interaction. Quest Diary and the NPC conversation can stay open;
ordinary world-click gating remains unchanged. Existing pending-operation
tracking prevents duplicate reward requests.

## Verification

- Shared-client `quest_` regression: 159/159 passed. Five new tests cover
  endpoint identity, rendered Finish/card, delayed correct/wrong NPC replies,
  twelve cancellation scenarios and remote return feedback. This filter also
  includes existing tests whose names contain `request_`.
- Simulation: the specific cave patrol public turn-in regression passes 1/1.
  Wrong/missing NPC conversation and excessive range cannot settle; Board's
  normal conversation permits completion and +500 gold, and reopening Board
  then replaying Finish does not duplicate rewards.
- The server test seeds prior completed quests and the two completed patrol
  objectives only in its isolated in-memory fixture. It does not claim to
  validate combat receipt generation or edit any live character.
- Native adapter/bridge regression: 95/95 passed, including packet-only Board
  and Peter, full-snapshot precedence, and the dedicated detail interaction's
  modal/death/notice/identity checks. Native atlas regression: 33/33 passed.
  Together with shared and server checks, 288 related tests pass. Native
  offline release build passes in 1m19s; physical UI verification is pending.

Logs use prefix `npc-turn-in-` under `C:/mir2-ui-repair-20260921`.
The first server test invocation omitted `--lib`, compiled unrelated integration
test binaries and exhausted E: build space. Fifty-five newly generated PDBs
(6.54 GiB) were reversibly relocated to
`C:/mir2-ui-repair-20260921/npc-turn-in-build-symbols`, with a manifest and size
checks. No source, game binary, asset or player store was removed. Subsequent
server verification targets the library test explicitly.

The old client PID49816 separately exited with rendering OOM at 21:32:27 UTC.
Its telemetry grows from 2 MiB/two font IDs at 10 seconds to 15,125 MiB/8,866
font IDs at 620 seconds. These are measured font-atlas allocations, distinct
from the cumulative local asset-resolution counter.
This repair does not resolve or accept that separate stability issue. No
native gameplay input, live store rewrite or live reward grant was issued.
Physical UI and original-client visual acceptance remain open.

## Deployment

Source `8d567e93ba18120c1e20e8ace5c806ad64bf3333` was committed and pushed.
Package: `C:/numeron-legend-of-rebirth-20260923-npc-turn-in`.

- Client SHA256: `0985D533BB3158110865081B50D3A855A557D64CAA213651C4C4D566FD7F3EFF`.
- Retained Gateway source `e65bdc4ebcd99b0c80658c087a1bf2f26c4cbc33`, SHA256
  `355E8461074A130C9E9B7CE2AA6039DC3DFBE25B2F58F8677EA29A74E7FD65D6`.
- Config SHA256: `01E4A40E54A5B8C7D6B1DABD7E4C13F7BEF434EC26EAEB1B77180E4BCEFEE834`.

After the user's continuation, both previous game processes and both listening
ports were absent. The existing account store was copied to a timestamped
backup and checked by hash; original store/profile/identity keys were retained.
The unmodified Gateway binary was started as PID32864 on 19900/19910 and
checked before launching client PID4196. No live process was forced closed.
The native window reports `numeron-legend of rebirth`; login/gameplay remains
user-controlled. Exact cache diagnostics remain enabled. Launch record:
`C:/mir2-ui-repair-20260921/render-live/npc-turn-in-client-launch.json`, log
prefix `20260923-165018-432`. `deployed=true`, `visualAccepted=false` and
`renderOomResolved=false` remain explicit in the package manifest.
