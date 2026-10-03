# envgg

This document contains the help content for the `envgg` command-line program.

**Command Overview:**

* [`envgg`↴](#envgg)
* [`envgg run`↴](#envgg-run)
* [`envgg secrets`↴](#envgg-secrets)
* [`envgg open`↴](#envgg-open)
* [`envgg vars`↴](#envgg-vars)
* [`envgg export`↴](#envgg-export)

## `envgg`

Run commands with environment variables from .env files, with secrets resolved from the system keyring

**Usage:** `envgg [OPTIONS] [ENV] -- <CMD>...
       envgg <COMMAND>`

Examples:
  envgg -- npm start                            # .env
  envgg development -- npm start                # .env.development
  envgg p -- tsx src/index.ts                   # .env.production
  envgg --env-file .my-unique-env -- npm start  # a specific env file
  envgg run p -- tsx src/index.ts               # same as without `run`

Exit codes when running a command:
  its own    the command ran
  125        envgg failed before running it
  126        the command was found but could not be run
  127        the command was not found

###### **Subcommands:**

* `run` — Run a command with the variables from a .env file (same as omitting `run`)
* `secrets` — List the secrets stored in the `envgg` namespace of the system keyring
* `open` — Open the GUI manager
* `vars` — Print the variable names used by the .env files in the current folder
* `export` — Write all secrets as plaintext to a file

###### **Arguments:**

* `<ENV>` — Environment to load from .env.<ENV>, also as d, s, p, t or l [default: .env]

  Possible values: `development`, `staging`, `production`, `test`, `local`

* `<CMD>` — Command and arguments to run (after `--`)

###### **Options:**

* `--env-file <FILE>` — Load this env file instead of .env or .env.<ENV>



## `envgg run`

Run a command with the variables from a .env file (same as omitting `run`)

**Usage:** `envgg run [OPTIONS] [ENV] -- <CMD>...`

###### **Arguments:**

* `<ENV>` — Environment to load from .env.<ENV>, also as d, s, p, t or l [default: .env]

  Possible values: `development`, `staging`, `production`, `test`, `local`

* `<CMD>` — Command and arguments to run (after `--`)

###### **Options:**

* `--env-file <FILE>` — Load this env file instead of .env or .env.<ENV>



## `envgg secrets`

List the secrets stored in the `envgg` namespace of the system keyring

**Usage:** `envgg secrets`



## `envgg open`

Open the GUI manager

**Usage:** `envgg open`



## `envgg vars`

Print the variable names used by the .env files in the current folder

**Usage:** `envgg vars`



## `envgg export`

Write all secrets as plaintext to a file

**Usage:** `envgg export [OPTIONS] [FILE]`

###### **Arguments:**

* `<FILE>` — File to write

  Default value: `.env.bak`

###### **Options:**

* `-f`, `--force` — Overwrite the file if it already exists



