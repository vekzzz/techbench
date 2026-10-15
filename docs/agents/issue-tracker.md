# Issue tracker: GitHub

Issues and specs for this repo live as GitHub issues in [vekzzz/techbench](https://github.com/vekzzz/techbench), worked with the `gh` CLI. The existing specs and tickets from before the move stay in `.scratch/` as history.

## Conventions

- One feature per spec issue; its implementation tickets are separate issues that link to it.
- Triage state is a GitHub label (see `triage-labels.md`); the category role is the `bug` or `enhancement` label.
- Blocking is written in the issue body as `Blocked by: #12, #14`. A ticket is unblocked when every issue it lists is closed.
- Comments and conversation history are ordinary issue comments.

## When a skill says "publish to the issue tracker"

Create an issue: `gh issue create --repo vekzzz/techbench --title "<title>" --body-file <file> --label <labels>`.

## When a skill says "fetch the relevant ticket"

`gh issue view <number> --repo vekzzz/techbench --comments`. The user will normally pass the number.

## Wayfinding operations

Used by `/wayfinder`. The **map** is a pinned issue with the Notes / Decisions-so-far / Fog body; each **child** ticket is an issue.

- **Map**: an issue titled `Map: <effort>`, labelled `map`.
- **Child ticket**: an issue whose body holds the question, with a `Type: research|prototype|grilling|task` line, and `Blocked by:` as above.
- **Frontier**: open, unblocked, unclaimed child issues; lowest number first.
- **Claim**: add the `claimed` label before any work.
- **Resolve**: comment the answer under an `## Answer` heading, close the issue, then add a pointer (gist and link) to the map's Decisions-so-far.
