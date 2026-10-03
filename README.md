# envgg

Run commands with environment variables from `.env` files, with secrets resolved from your system keyring. A `.env` file lists a secret by name instead of containing its value, so it is safe to leave in your project.

## Install

With [cargo-binstall](https://github.com/cargo-bins/cargo-binstall):

```bash
cargo binstall --git https://github.com/ronanyeah/envgg envgg
```

Or download an archive for your platform from the [releases page](https://github.com/ronanyeah/envgg/releases).

## Usage

### Run a command

Everything after `--` is the command to run. An optional environment before it picks the env file:

```bash
envgg -- npm start                     # .env
envgg development -- npm start         # .env.development
envgg p -- tsx src/index.ts            # .env.production
envgg --env-file .my-env -- npm start  # any other env file
```

| Environment | Short | File |
|---|---|---|
| `development` | `d` | `.env.development` |
| `staging` | `s` | `.env.staging` |
| `production` | `p` | `.env.production` |
| `test` | `t` | `.env.test` |
| `local` | `l` | `.env.local` |

The command's exit code is passed through. On Linux and macOS it also replaces envgg, so signals reach it directly. `envgg run` is an optional spelling of the same thing: `envgg run p -- npm start`.

If the command can't be run, envgg exits with 127 (not found) or 126 (not executable). It exits with 125 if envgg itself fails first, for example if the env file is missing.

### Manage secrets

```bash
envgg secrets      # list the secrets in the keyring
envgg set NAME     # add or update a secret
envgg delete NAME  # delete a secret (-y to skip the confirmation)
envgg open         # open the GUI manager
```

`set` prompts for the value, or reads it from stdin when piped (`printf %s "$VALUE" | envgg set NAME`). The value is never taken as an argument, so it stays out of your shell history.

### Inspect and back up

```bash
envgg vars    # list the variable names used by the .env files in this folder
envgg export  # write all secrets as plaintext to .env.bak (-f to overwrite)
```

See [CLI.md](docs/CLI.md) for the full command reference.

## Env file format

```bash
# comment - will be ignored
FOO=123    [will be exported]
APP_SECRET [will be sourced from device keyring]
APP_SECRET=$ALIAS [ALIAS will be sourced from device keyring, and exported as APP_SECRET]
```

Quoted values can span multiple lines:

```bash
PRIVATE_KEY="-----BEGIN KEY-----
...
-----END KEY-----"
```
