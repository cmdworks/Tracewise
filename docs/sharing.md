# Sharing

By default Tracewise history stays on your machine. Sharing lets every clone of a project — your laptop, your desktop, your teammates — see the same history: who changed what and why, on every machine.

## How it works

- Each machine writes its new records as an append-only **batch** and pushes it to a git remote under its own ref (`refs/trail/device/<id>`). Machines never write to each other's refs, so pushes never conflict.
- Fetching imports other machines' batches into the local database. The database itself is never pushed; it can always be rebuilt from the batches with `tracewise rebuild`.
- Every batch is **signed** with a key of the machine that made it. The first batch from a machine pins its key; a batch that doesn't match is refused.
- Locks are never shared.

## Set it up

Sharing is turned on per project, in `.trailconfig`, so everyone who clones the project gets the same setting.

**The simplest setup uses the project's own remote:**

```bash
tracewise share init                 # uses origin, hidden refs, level "summary"
git add .trailconfig && git commit -m "Share Tracewise history"
git push
```

The batches go to the same repository as your code, under hidden refs. They do not show up as branches, and nobody needs a second repository.

**Or use a separate repository** — useful when the history should be readable by fewer people than the code:

```bash
tracewise share init --remote git@github.com:me/my-project-history.git
```

Then push and fetch:

```bash
tracewise push        # send this machine's new records
tracewise fetch       # import everyone else's
```

With the git wrapper installed, `git fetch`, `git pull` and `git clone` also fetch Tracewise history.

**On every other clone**, approve the remote once — Tracewise never pushes to or fetches from a remote named in a `.trailconfig` you have not approved:

```bash
tracewise share trust
tracewise fetch
```

`tracewise share status` shows the whole setup at any time.

## What is shared: levels

| Level | Shared |
|---|---|
| `summary` *(default)* | who changed which file when, the changed lines, reasons, and the content of each file at each commit |
| `full` | also every intermediate version of every file, and agents' prompts |
| `off` | nothing |

```bash
tracewise share init --level full
```

Even at `full`, files with secret-looking names (`.env*`, `*.pem`, `*.key`, `id_*`, …) are shared without content, and files containing well-known credentials are left out.

> **On a public repository, everyone can read what you share.** Use `summary`, a private history repository, or encryption.

## Encryption

Encryption makes shared history unreadable to anyone without the project key — including whoever hosts the remote.

```bash
tracewise share init --encrypt
```

This creates a random project key on this machine (XChaCha20-Poly1305). It is stored in the repository's private data folder and is **never pushed**. Every batch is encrypted before it leaves the machine; only the time range of each batch stays readable.

**Hand the key to every other clone yourself:**

```bash
tracewise share key export        # on a machine that has the key: prints trailkey1:…
tracewise share key import 'trailkey1:…'   # on each other clone
```

The exported string *is* the key. Keep a copy in a password manager: a clone that loses its key cannot read the shared history, and `tracewise doctor` warns until the key has been exported once.

**Managing keys:**

| Command | When |
|---|---|
| `tracewise share key list` | see which keys this clone has (never prints a key) |
| `tracewise share key rotate` | start using a new key for new batches; old batches stay readable with the old key. Hand the new key out with `export` / `import` |
| `tracewise share key rekey` | a key leaked or was lost: rotate, then push this machine's whole history again under the new key. Add `--prune-old --yes` to delete the old batches from the remote first |

There is no passphrase: the key file is readable by you only, like an SSH key. Agents cannot export, import or change keys.

## Choosing a setup

| You want | Use |
|---|---|
| Your own machines, private repository | `tracewise share init` |
| A team, private repository | `tracewise share init`, everyone runs `share trust` |
| A public or widely shared repository | `tracewise share init --encrypt`, or `--remote` to a private history repository |
| Full intermediate versions on a private remote | `tracewise share init --level full` |
