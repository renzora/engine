# Collaborating on a Scene

Two people, two copies of Renzora, one scene. One of you hosts the session; the
other joins it, sees the project appear in their editor, and — once you hand
over control — builds alongside you.

Open the **Collaborate** panel from the dock's **+** picker (under *Session*).

There are two ways to connect, and the panel offers both.

| | Online session | Direct connection |
|---|---|---|
| Reaches | Anyone, anywhere | Your own network, or a forwarded port |
| Needs | A renzora.com account | Nothing |
| You share | An 8-character code | An IP address and port |
| Traffic goes | Through renzora.com | Straight between the two machines |

**Use an online session unless you have a reason not to.** Two people on ordinary
home connections cannot reach each other directly — both are behind NAT — which
is exactly the problem the relay solves. Direct is there for a LAN, and for
anyone who would rather their project never touch a third machine.

## Hosting an online session

1. Sign in to renzora.com (Account panel).
2. Open the project you want to work on.
3. In **Collaborate**, press **Start an online session**.

The panel shows a code like `K7MPQ2XZ`. Send that to your collaborator — it is
all they need.

To invite someone from your friends list instead, they get a notification with a
one-click join. Invitations are friends-only; the code works for anyone you give
it to.

## Hosting a direct connection

1. Open the project you want to work on.
2. In **Collaborate**, set **Your name** (this is what your collaborator sees).
3. Press **Start hosting** under *Or connect directly*.

The panel then shows an address like `192.168.1.20:7700`. Read that to your
collaborator — it is what they type in to join.

Guests start out **watching**. The button under the address says so, and clicking
it flips between:

| State | What a guest can do |
|---|---|
| *Guests are watching* | Move their own camera, look around, select things. Their edits stay on their machine. |
| *Guests can edit* | Everything above, plus changes that land in your scene. |

You can flip it mid-session, as often as you like. It is deliberately off at the
start — "let me show you something" is a much more common invitation than "take
the wheel".

## Joining

1. Open **any** project (see the warning below).
2. Either paste the code into **Or join with a code** and press **Join session**,
   or — for a direct connection — type the host's address into **Host address**
   and press **Join**.

The host's scene replaces what you had open, and their file list arrives a moment
later. If you are missing any of their models, textures or scripts, the panel
offers to fetch them and shows how much that is; nothing is downloaded until you
press the button.

> **Joining replaces your open scene.** Everything in the scene you had open is
> despawned and the host's is put in its place. Save your own work before
> joining. Your project's *files* are never deleted — only added to — but the
> scene in the viewport is the host's from the moment you connect.

## What you see

Each person in the session gets a colour, used consistently everywhere:

- A **camera marker** in the viewport, pointing the way they are looking.
- An **outline** around whatever they have selected.
- A **dot** beside their name in the panel.

Selecting something also claims it. If a collaborator already has an object
selected, it is theirs for as long as they hold it — you can still move it, but
their version wins. The panel shows who is editing how many objects.

## Reaching each other

**Online sessions** work anywhere. Both editors connect outward to renzora.com,
which every network allows, so neither of you needs to be reachable. Traffic is
encrypted in transit (WSS) and the relay forwards it without reading it — but it
does pass through renzora.com's servers, which is the trade for it working at
all.

**Direct connections** need the guest to be able to reach the host:

- **Same network** (same house, same office, same Wi-Fi) — works with no setup.
  Use the address the panel shows.
- **Over the internet** — needs the host to forward port `7700` on their router,
  or a VPN/tunnel between the two machines.

> A direct connection is **not encrypted**. On a home or office network that is
> fine. Do not forward the port to the open internet and expect privacy — use an
> online session instead.

## Codes

A code is the invitation. Anyone signed in who has it can join, whether you
added them as a friend or not — the same way a meeting link works. Codes are
random, unguessable, and last as long as the session (up to 12 hours). Ending
the session invalidates the code immediately.

You can host up to three sessions at once on one account.

## What syncs, and what doesn't

**Syncs:**

- Entities — spawning, deleting, renaming, reparenting
- Transforms — moving, rotating, scaling, from the gizmo or the inspector
- Components — adding, removing, and every field you can edit on them
- Lights, cameras, models, sprites, and anything else built out of entities
- Project files, on request

**Doesn't sync yet:**

- Terrain sculpting and painting
- Tilemap painting
- Material and blueprint graphs
- Script and code-editor buffers
- Play mode — pressing Play runs the game on your machine only

Each of those carries data that does not fit an entity snapshot and needs its own
channel; they are planned, not forgotten.

## Undo

Ctrl+Z undoes **your** actions. It cannot reach a collaborator's work, which is
the safe half of the behaviour — nobody can rewind something you just did. When
you undo, the result travels to the others as an ordinary change.

## Saving

The host's copy is the real one. The host saves the scene as usual (Ctrl+S) and
that file is the session's result. A guest saving writes to their own copy of the
project, which is useful as a snapshot but is not what the host has.

## When something goes wrong

| What you see | What it means |
|---|---|
| *Sign in to renzora.com to host an online session* | Online sessions authenticate everyone, so the host sees real names rather than addresses. Sign in from the Account panel. |
| *No session with that code* | Mistyped, or the session has ended. Codes exclude `0/O` and `1/I/L` to make them easier to read out. |
| *That session already has a host connected* | You are already hosting it from another editor. |
| *Could not host on port 7700* | Something else is using the port, or the OS refused it. Change the port and try again. |
| *Could not reach …* | Wrong address, the host has not started hosting, or a firewall is in the way. |
| *Host speaks protocol N* | The two editors are different versions. Both must be rebuilt from the same commit. |
| *stream desynchronised* | A bug on this side of the link, not a bad peer — please report it with the surrounding log lines. |
| *Refused an unsafe path* | The other side sent a file path that pointed outside the project. It was ignored — nothing was written. |
| A collaborator's edits stop arriving | Check the panel: if they dropped, the activity list says so. |

The **Activity** list at the bottom of the panel is a running account of the
session — who joined, what was sent, what failed.
