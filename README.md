# Board Bases

Syncs GitHub Projects boards into Obsidian as notes plus a Bases kanban view, both ways: drag a card in Obsidian and the GitHub status follows.

## Usage

    board-bases sync [--dry-run] [--out DIR] [--config FILE]

- `--config` defaults to `$BOARD_BASES_CONFIG` or `~/.config/board-bases/config.json`. See `config.example.json` for the 12 boards.
- The output folder is `--out`, then `$BOARD_BASES_OUT`, then the config's `output`, then `~/Documents/Citadel/Boards`.
- `--dry-run` reads GitHub and prints what would change; it writes nothing and pushes nothing.

It needs `gh` authenticated with the `project` scope. Each run is one short process with about ten GitHub calls, and nothing runs in the background.

## What syncs

- One note per issue, PR or draft: `notes/<owner>/<repo> <number>.md`, `notes/drafts/<id>.md`. Frontmatter: title, status, repo, number, type, state, url, boards, updated.
- Only `status` is two-way. Other managed fields are rewritten from GitHub. Extra frontmatter keys and the body are never touched.
- One `<board name>.base` per board, filtered on that board: a table view grouped by status.

## Conflicts

Each note's last-synced state lives in `<output>/.board-bases/state.json`. A three-way merge compares the note and every board against it:

- Vault-only status change: pushed to every board holding the item.
- GitHub-only change: the note is rewritten.
- Both changed to different values: GitHub wins, the note is rewritten, and a `CONFLICT` line is printed.
- A status that is not one of the four is left alone with a `WARN` line.
- An item removed from every board moves its note to `<output>/.trash/`. Nothing is deleted.

Syncing twice in a row with no changes writes nothing.

## Obsidian

The kanban view type is not emitted: it could not be verified (the installed build, 1.9.12, has no kanban type). Revisit once the 1.14.5 syntax is confirmed.

MIT licensed.
